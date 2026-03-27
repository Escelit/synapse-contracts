#![no_std]

extern crate alloc;

mod access;
mod events;
mod storage;
pub mod types;

use access::{require_admin, require_not_paused, require_relayer};
use events::emit;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, Env, String as SorobanString, Symbol, Vec};
use storage::{assets, deposits, dlq, max_deposit, min_deposit, relayers, settlements};
use types::{DlqEntry, Event, Settlement, Transaction, TransactionStatus, MAX_RETRIES};

#[contract]
pub struct SynapseContract;

fn next_id(env: &Env, counter_key: Symbol) -> SorobanString {
    let nonce: u32 = env.storage().instance().get(&counter_key).unwrap_or(0);
    env.storage().instance().set(&counter_key, &(nonce + 1));

    let ts = env.ledger().timestamp();
    let seq = env.ledger().sequence();

    let mut data = [0u8; 16];
    data[..8].copy_from_slice(&ts.to_be_bytes());
    data[8..12].copy_from_slice(&seq.to_be_bytes());
    data[12..16].copy_from_slice(&nonce.to_be_bytes());

    let hash = env.crypto().sha256(&Bytes::from_slice(env, &data));
    let bytes = hash.to_array();

    let mut hex = [0u8; 32];
    const HEX: &[u8] = b"0123456789abcdef";
    for i in 0..16 {
        hex[i * 2] = HEX[(bytes[i] >> 4) as usize];
        hex[i * 2 + 1] = HEX[(bytes[i] & 0xf) as usize];
    }
    SorobanString::from_bytes(env, &hex)
}

#[contractimpl]
impl SynapseContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&storage::StorageKey::Admin) {
            panic!("already initialised");
        }
        admin.require_auth();
        storage::admin::set(&env, &admin);
        emit(&env, Event::Initialized(admin));
    }

    pub fn grant_relayer(env: Env, caller: Address, relayer: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        relayers::add(&env, &relayer);
        emit(&env, Event::RelayerGranted(relayer));
    }

    pub fn revoke_relayer(env: Env, caller: Address, relayer: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if !relayers::has(&env, &relayer) { panic!("address is not a relayer") }
        relayers::remove(&env, &relayer);
    }

    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        let old_admin = storage::admin::get(&env);
        storage::admin::set(&env, &new_admin);
        emit(&env, Event::AdminTransferred(old_admin, new_admin));
    }

    pub fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        let current_admin = storage::admin::get(&env);
        storage::pending_admin::set(&env, &new_admin);
        emit(&env, Event::AdminTransferProposed(current_admin, new_admin));
    }

    pub fn accept_admin(env: Env, caller: Address) {
        caller.require_auth();
        let pending = storage::pending_admin::get(&env).expect("no pending admin");
        if caller != pending {
            panic!("not pending admin")
        }
        let old_admin = storage::admin::get(&env);
        storage::admin::set(&env, &pending);
        storage::pending_admin::clear(&env);
        emit(&env, Event::AdminTransferred(old_admin, pending));
    }

    pub fn pause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, true);
        emit(&env, Event::ContractPaused(caller));
    }

    pub fn unpause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, false);
        emit(&env, Event::ContractUnpaused(caller));
    }

    pub fn add_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if asset_code.len() == 0 {
            panic!("invalid asset code")
        }
        let mut buf = [0u8; 12];
        let len = asset_code.len() as usize;
        if len > buf.len() {
            panic!("invalid asset code")
        }
        asset_code.copy_into_slice(&mut buf[..len]);
        for b in &buf[..len] {
            if !b.is_ascii_uppercase() && !b.is_ascii_digit() {
                panic!("invalid asset code")
            }
        }
        assets::add(&env, &asset_code);
        emit(&env, Event::AssetAdded(asset_code));
    }

    pub fn remove_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if !assets::is_allowed(&env, &asset_code) {
            panic!("asset not in allowlist");
        }
        assets::remove(&env, &asset_code);
        emit(&env, Event::AssetRemoved(asset_code));
    }

    pub fn set_min_deposit(env: Env, caller: Address, amount: i128) {
        require_admin(&env, &caller);
        if amount <= 0 { panic!("min deposit must be positive") }
        min_deposit::set(&env, amount);
    }

    pub fn get_min_deposit(env: Env) -> Option<i128> {
        min_deposit::get(&env)
    }

    pub fn set_max_deposit(env: Env, caller: Address, amount: i128) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if amount <= 0 {
            panic!("max deposit must be positive")
        }
        max_deposit::set(&env, &amount);
    }

    pub fn get_max_deposit(env: Env) -> i128 {
        max_deposit::get(&env).unwrap_or(0)
    }

    pub fn register_deposit(
        env: Env,
        caller: Address,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        amount: i128,
        asset_code: SorobanString,
        memo: Option<SorobanString>,
        memo_type: Option<SorobanString>,
    ) -> SorobanString {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        if anchor_transaction_id.len() == 0 { panic!("anchor_transaction_id must not be empty") }
        assets::require_allowed(&env, &asset_code);

        if let Some(min) = min_deposit::get(&env) {
            if amount < min { panic!("amount below min deposit") }
        }
        if let Some(max) = max_deposit::get(&env) {
            if amount > max { panic!("amount exceeds max deposit") }
        }

        if let Some(existing) = deposits::find_by_anchor_id(&env, &anchor_transaction_id) {
            return existing;
        }

        let tx = Transaction::new(
            &env,
            anchor_transaction_id.clone(),
            stellar_account,
            caller,
            amount,
            asset_code,
            memo,
            memo_type,
            None,
        );
        let id = tx.id.clone();
        deposits::save(&env, &tx);
        deposits::index_anchor_id(&env, &anchor_transaction_id, &id);
        emit(&env, Event::DepositRegistered(id.clone(), anchor_transaction_id));
        id
    }

    pub fn mark_processing(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        if tx.status != TransactionStatus::Pending {
            panic!("transaction must be Pending");
        }
        let new_status = TransactionStatus::Processing;
        tx.status = new_status.clone();
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, new_status));
    }

    pub fn mark_completed(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        if tx.status != TransactionStatus::Processing {
            panic!("transaction must be Processing");
        }
        let new_status = TransactionStatus::Completed;
        tx.status = new_status.clone();
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        if dlq::get(&env, &tx_id).is_some() {
            dlq::remove(&env, &tx_id);
        }
        emit(&env, Event::StatusUpdated(tx_id, new_status));
    }

    pub fn mark_failed(
        env: Env,
        caller: Address,
        tx_id: SorobanString,
        error_reason: SorobanString,
    ) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        if error_reason.len() == 0 {
            panic!("error_reason must not be empty");
        }
        let mut tx = deposits::get(&env, &tx_id);
        if tx.status == TransactionStatus::Completed {
            panic!("cannot fail completed transaction");
        }
        let new_status = TransactionStatus::Failed;
        tx.status = new_status.clone();
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id.clone(), new_status));
        let entry = DlqEntry::new(&env, tx_id.clone(), error_reason.clone());
        dlq::push(&env, &entry);
        emit(&env, Event::MovedToDlq(tx_id, error_reason));
    }

    pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        caller.require_auth();
        let mut entry = dlq::get(&env, &tx_id).expect("dlq entry not found");
        let mut tx = deposits::get(&env, &tx_id);
        let is_admin = caller == storage::admin::get(&env);
        let is_original_relayer = caller == tx.relayer;
        if !is_admin && !is_original_relayer {
            panic!("not admin")
        }
        if entry.retry_count >= MAX_RETRIES {
            emit(&env, Event::MaxRetriesExceeded(tx_id.clone()));
            panic!("max retries exceeded");
        }
        entry.retry_count += 1;
        entry.last_retry_ledger = env.ledger().sequence();
        dlq::push(&env, &entry);
        tx.status = TransactionStatus::Pending;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::DlqRetried(tx_id.clone()));
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Pending));
    }

    pub fn cancel_transaction(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Cancelled;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Cancelled));
    }

    pub fn finalize_settlement(
        env: Env,
        caller: Address,
        asset_code: SorobanString,
        tx_ids: Vec<SorobanString>,
        total_amount: i128,
        period_start: u64,
        period_end: u64,
    ) -> SorobanString {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        if period_start > period_end {
            panic!("period_start must be <= period_end")
        }
        let settlement_id = next_id(&env, symbol_short!("stlnonce"));
        let s = Settlement::new(
            &env,
            settlement_id,
            asset_code.clone(),
            tx_ids.clone(),
            total_amount,
            period_start,
            period_end,
        );
        let id = s.id.clone();
        let n = tx_ids.len();
        let mut i: u32 = 0;
        while i < n {
            let tx_id = tx_ids.get(i).unwrap();
            let mut tx = deposits::get(&env, &tx_id);
            if tx.settlement_id.len() > 0 {
                panic!("transaction already settled");
            }
            if tx.status != TransactionStatus::Completed {
                panic!("transaction not completed");
            }
            tx.settlement_id = id.clone();
            tx.updated_ledger = env.ledger().sequence();
            deposits::save(&env, &tx);
            emit(&env, Event::Settled(tx_id, id.clone()));
            i += 1;
        }
        settlements::save(&env, &s);
        emit(&env, Event::SettlementFinalized(id.clone(), asset_code, total_amount));
        id
    }

    pub fn get_dlq_entry(env: Env, tx_id: SorobanString) -> Option<DlqEntry> {
        dlq::get(&env, &tx_id)
    }

    pub fn get_admin(env: Env) -> Address {
        storage::admin::get(&env)
    }

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        storage::pending_admin::get(&env)
    }

    pub fn is_paused(env: Env) -> bool {
        storage::pause::is_paused(&env)
    }

    pub fn get_transaction(env: Env, tx_id: SorobanString) -> Transaction {
        deposits::get(&env, &tx_id)
    }

    pub fn get_settlement(env: Env, settlement_id: SorobanString) -> Settlement {
        settlements::get(&env, &settlement_id)
    }

    pub fn is_asset_allowed(env: Env, asset_code: SorobanString) -> bool {
        assets::is_allowed(&env, &asset_code)
    }

    pub fn is_relayer(env: Env, address: Address) -> bool {
        relayers::has(&env, &address)
    }
}
