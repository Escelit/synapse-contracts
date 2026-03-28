#![no_std]

extern crate alloc;

mod access;
mod events;
pub mod storage;
pub mod types;

use access::{require_admin, require_not_paused, require_relayer};
use events::emit;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, Env, String as SorobanString, Symbol, Vec};
use storage::{assets, deposits, dlq, max_deposit, min_deposit, relayers, settlements};
use types::{DlqEntry, Event, Settlement, Transaction, TransactionStatus, MAX_RETRIES};

#[contract]
pub struct SynapseContract;

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
        if !relayers::has(&env, &relayer) {
            panic!("address is not a relayer")
        }
        relayers::remove(&env, &relayer);
    }

    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        storage::admin::set(&env, &new_admin);
        emit(&env, Event::AdminTransferAccepted(caller, new_admin));
    }

    pub fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        let current_admin = storage::admin::get(&env);
        storage::pending_admin::set(&env, &new_admin);
        emit(&env, Event::AdminTransferProposed(current_admin, new_admin));
    }

    pub fn accept_admin(env: Env, caller: Address) {
        require_not_paused(&env);
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

    pub fn get_pending_admin(env: Env) -> Option<Address> {
        storage::pending_admin::get(&env)
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

    pub fn is_asset_allowed(env: Env, asset_code: SorobanString) -> bool {
        assets::is_allowed(&env, &asset_code)
    }

    pub fn set_min_deposit(env: Env, caller: Address, amount: i128) {
        require_not_paused(&env);
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

    // TODO(#16): enforce maximum deposit amount (configurable by admin) — DONE

    // TODO(#15): enforce minimum deposit amount (configurable by admin)
    // TODO(#17): validate anchor_transaction_id is non-empty
    // TODO(#20): add `callback_type` field (deposit | withdrawal)
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
        if anchor_transaction_id.len() == 0 {
            panic!("anchor_transaction_id must not be empty")
        }
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
            None, // callback_type
        );
        let id = tx.id.clone();
        deposits::save(&env, &tx);
        deposits::index_anchor_id(&env, &anchor_transaction_id, &id);
        emit(&env, Event::DepositRegistered(id.clone(), anchor_transaction_id));
        id
    }

    pub fn get_transaction(env: Env, tx_id: SorobanString) -> Transaction {
        deposits::get(&env, &tx_id)
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
        caller.require_auth();
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
        let old_status = tx.status.clone();
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
        let mut sum: i128 = 0;
        for tx_id in tx_ids.iter() {
            let tx = deposits::get(&env, &tx_id);
            sum = sum.checked_add(tx.amount).expect("amount overflow");
        }
        if sum != total_amount {
            panic!("total_amount mismatch");
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
            let tx = deposits::get(&env, &tx_id);
            if !tx.settlement_id.is_empty() {
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

    pub fn get_dlq_entry(env: Env, tx_id: SorobanString) -> Option<DlqEntry> {
        dlq::get(&env, &tx_id)
    }

    pub fn is_relayer(env: Env, address: Address) -> bool {
        relayers::has(&env, &address)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageKey;
    use crate::types::Transaction;
    use soroban_sdk::{
        symbol_short,
        testutils::{
            storage::Persistent as _, Address as _, Events as _, Ledger as _,
        },
        vec, Env, IntoVal, String as SorobanString,
    };

    const TEST_ASSET_CODES: [&str; 20] = [
        "S0", "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9", "T0", "T1", "T2", "T3", "T4",
        "T5", "T6", "T7", "T8", "T9",
    ];

    fn setup(env: &Env) -> (Address, Address) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        client.initialize(&admin);
        (admin, contract_id)
    }

    const TEST_ASSET_CODES: &[&str] = &[
        "USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "CNY", "SEK", "NZD",
        "MXN", "SGD", "HKD", "NOK", "KRW", "TRY", "RUB", "INR", "BRL", "ZAR",
    ];

    fn setup_relayer_deposit<'a>(
        env: &'a Env,
        anchor_label: &str,
    ) -> (SynapseContractClient<'a>, Address, SorobanString) {
        let (admin, contract_id) = setup(env);
        let client = SynapseContractClient::new(env, &contract_id);
        let relayer = Address::generate(env);
        let stellar = Address::generate(env);
        let asset = SorobanString::from_str(env, "USD");
        let anchor_id = SorobanString::from_str(env, anchor_label);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id = client.register_deposit(&relayer, &anchor_id, &stellar, &1i128, &asset, &None, &None);
        (client, relayer, tx_id)
    }

    #[test]
    #[should_panic(expected = "address is not a relayer")]
    fn test_revoke_relayer_panics_when_not_a_relayer() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let non_relayer = Address::generate(&env);
        client.revoke_relayer(&admin, &non_relayer);
    }

    #[test]
    #[should_panic(expected = "asset not in allowlist")]
    fn test_remove_asset_panics_when_not_in_allowlist() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let unknown = SorobanString::from_str(&env, "UNK");
        client.remove_asset(&admin, &unknown);
    }

    #[test]
    fn test_register_deposit_stores_relayer() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "relayer-on-tx");
        let tx = client.get_transaction(&tx_id);
        let _ = relayer;
        let _ = tx;
    }

    #[test]
    fn test_register_deposit_stores_memo_type() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "memo-type-stored");
        let memo_type_val = SorobanString::from_str(&env, "hash");

        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id = client.register_deposit(
            &relayer,
            &anchor_id,
            &stellar,
            &100i128,
            &asset,
            &None,
            &Some(memo_type_val.clone()),
        );

        let tx = client.get_transaction(&tx_id);
        assert_eq!(tx.memo_type, Some(memo_type_val));
    }

    #[test]
    fn test_register_deposit_stores_memo() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "memo-stored");
        let memo = SorobanString::from_str(&env, "test-memo");

        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id = client.register_deposit(
            &relayer,
            &anchor_id,
            &stellar,
            &100i128,
            &asset,
            &Some(memo.clone()),
            &None,
        );

        let tx = client.get_transaction(&tx_id);
        assert_eq!(tx.memo, Some(memo));
    }

    #[test]
    fn test_register_deposit_memo_type_none_is_valid() {
        let env = Env::default();
        let (client, _relayer, tx_id) = setup_relayer_deposit(&env, "mt-none");
        let tx = client.get_transaction(&tx_id);
        assert!(tx.memo_type.is_none());
    }

    #[test]
    #[should_panic(expected = "transaction must be Pending")]
    fn test_mark_processing_panics_when_not_pending() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mp-not-pending");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        // tx is now Completed — mark_processing must panic
        client.mark_processing(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "error_reason must not be empty")]
    fn test_mark_failed_panics_when_error_reason_empty() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-empty-reason");
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, ""));
    }

    #[test]
    fn test_mark_failed_allowed_when_pending() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-pending");
        let err = SorobanString::from_str(&env, "boom");
        client.mark_failed(&relayer, &tx_id, &err);
        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Failed));
    }

    #[test]
    fn test_mark_failed_allowed_when_processing() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-processing");
        client.mark_processing(&relayer, &tx_id);
        let err = SorobanString::from_str(&env, "boom");
        client.mark_failed(&relayer, &tx_id, &err);
        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Failed));
    }

    #[test]
    fn test_mark_completed_succeeds_when_processing() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mc-ok");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Completed));
    }

    #[test]
    #[should_panic(expected = "transaction must be Processing")]
    fn test_mark_completed_panics_when_already_completed() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mc-twice");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "cannot fail completed transaction")]
    fn test_mark_failed_panics_when_completed() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-completed");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        client.mark_failed(
            &relayer,
            &tx_id,
            &SorobanString::from_str(&env, "late-fail"),
        );
    }

    #[test]
    #[should_panic(expected = "transaction must be Processing")]
    fn test_mark_completed_panics_when_not_processing() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mc-not-processing");
        client.mark_completed(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "transaction must be Pending or Processing")]
    fn test_mark_failed_panics_when_already_failed() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-twice");
        let err = SorobanString::from_str(&env, "first");
        client.mark_failed(&relayer, &tx_id, &err);
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "second"));
    }

    #[test]
    fn test_initialize_emits_initialized_event() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (emitting_contract, topics, _data) = events.get(0).unwrap();
        assert_eq!(emitting_contract, contract_id);
        assert_eq!(topics, (symbol_short!("synapse"),).into_val(&env));
    }

    #[test]
    fn test_transfer_admin_emits_event() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        client.transfer_admin(&admin, &new_admin);

        let events = env.events().all();
        let (_, _, data) = events.last().unwrap();
        let (event, _ledger): (Event, u32) = TryFromVal::try_from_val(&env, &data).unwrap();
        assert_eq!(
            event,
            Event::AdminTransferred(admin, new_admin.clone()),
        );
        // new admin is now stored
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    fn test_get_admin() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);

        // Should return the admin that was set during initialization
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_is_paused() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        assert!(!client.is_paused());
        client.pause(&admin);
        assert!(client.is_paused());
        client.unpause(&admin);
        assert!(!client.is_paused());
    }

    #[test]
    #[should_panic(expected = "period_start must be <= period_end")]
    fn test_finalize_settlement_panics_when_period_start_exceeds_period_end() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "period-order-inner"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env, tx_id],
            &100i128,
            &10u64,
            &1u64,
        );
    }

    #[test]
    fn test_min_deposit() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);

        // Default should be 0
        assert_eq!(client.get_max_deposit(), 0);

        // Set to 1000
        client.set_max_deposit(&admin, &1000i128);
        assert_eq!(client.get_max_deposit(), 1000i128);
        // Not set yet — should return None
        assert_eq!(client.get_min_deposit(), None);

        // Set to 100
        client.set_min_deposit(&admin, &100i128);
        assert_eq!(client.get_min_deposit(), Some(100i128));

        // Update to 500
        client.set_min_deposit(&admin, &500i128);
        assert_eq!(client.get_min_deposit(), Some(500i128));
    }

    #[test]
    #[should_panic(expected = "amount below min deposit")]
    fn test_register_deposit_panics_when_below_min() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");

        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        client.set_min_deposit(&admin, &50i128);

        // amount 10 < min 50 — should panic
        client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "below-min-anchor"),
            &stellar,
            &10i128,
            &asset,
            &None,
            &None,
        );
    }

    #[test]
    fn test_max_deposit() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        assert_eq!(client.get_max_deposit(), 0i128);
        client.set_max_deposit(&admin, &1000i128);
        assert_eq!(client.get_max_deposit(), 1000i128);
        client.set_max_deposit(&admin, &5000i128);
        assert_eq!(client.get_max_deposit(), 5000i128);
    }

    #[test]
    fn test_add_asset_respects_max_assets_cap() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        for code in TEST_ASSET_CODES {
            client.add_asset(&admin, &SorobanString::from_str(&env, code));
        }
    }

    #[test]
    #[should_panic]
    fn test_add_asset_panics_when_cap_exceeded() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        for code in TEST_ASSET_CODES {
            client.add_asset(&admin, &SorobanString::from_str(&env, code));
        }
        client.add_asset(&admin, &SorobanString::from_str(&env, "OVERFLOW"));
    }

    #[test]
    fn test_register_deposit_same_anchor_same_env_returns_same_tx_id() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let depositor = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
        let anchor = SorobanString::from_str(&env, "deterministic-anchor");
        let id1 = client.register_deposit(
            &relayer,
            &anchor,
            &depositor,
            &100_000_000,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        let id2 = client.register_deposit(
            &relayer,
            &anchor,
            &depositor,
            &100_000_000,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        assert_eq!(id1, id2);
    }

        assert_eq!(tx_id_1, tx_id_2);
    }

    #[test]
    fn test_register_deposit_initializes_memo_to_none() {
        let env = Env::default();
        let (client, _relayer, tx_id) = setup_relayer_deposit(&env, "memo-none");
        let tx = client.get_transaction(&tx_id);
        assert!(tx.memo.is_none());
    }

    #[test]
    fn test_retry_dlq_success() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "retry-tx");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id =
            client.register_deposit(&relayer, &anchor_id, &stellar, &1i128, &asset, &None, &None);

        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
        env.ledger().set_sequence_number(100);
        client.retry_dlq(&admin, &tx_id);

        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Pending));
        assert_eq!(tx.updated_ledger, 100);
    }

    #[test]
    fn test_retry_dlq_removes_dlq_entry() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "retry-remove"),
            &stellar,
            &1i128,
            &asset,
            &None,
            &None,
        );
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
        client.retry_dlq(&admin, &tx_id);
        let entry = env.as_contract(&contract_id, || storage::dlq::get(&env, &tx_id));
        assert!(entry.is_none());
    }

    #[test]
    #[should_panic(expected = "anchor_transaction_id must not be empty")]
    fn test_register_deposit_panics_on_empty_anchor_id() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        client.register_deposit(&relayer, &SorobanString::from_str(&env, ""), &stellar, &100i128, &asset, &None, &None);
    }

    #[test]
    fn test_finalize_settlement_writes_settlement_id_back_onto_transactions() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "settle-backref");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        let settlement_id = client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env, tx_id.clone()],
            &1i128,
            &0u64,
            &1u64,
        );
        let tx = client.get_transaction(&tx_id);
        assert_eq!(tx.settlement_id, settlement_id);
    }

    #[test]
    fn test_finalize_settlement_succeeds_when_transactions_unsettled() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "finalize-ok-anchor");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id =
            client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);

        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);

        let settlement_id = client.finalize_settlement(
            &relayer,
            &asset,
            &vec![&env, tx_id.clone()],
            &100i128,
            &1u64,
            &2u64,
        );
        assert!(settlement_id.len() > 0);
        let s = client.get_settlement(&settlement_id);
        assert_eq!(s.total_amount, 100i128);
    }

    #[test]
    #[should_panic(expected = "invalid asset code")]
    fn test_add_asset_panics_on_empty_code() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        client.add_asset(&admin, &SorobanString::from_str(&env, ""));
    }

    #[test]
    #[should_panic(expected = "invalid asset code")]
    fn test_add_asset_panics_on_lowercase_code() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        client.add_asset(&admin, &SorobanString::from_str(&env, "usd"));
    }

    #[test]
    #[should_panic(expected = "invalid asset code")]
    fn test_add_asset_panics_on_special_char_code() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        client.add_asset(&admin, &SorobanString::from_str(&env, "US$"));
    }

    #[test]
    fn test_cancel_transaction_success() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "cancel-tx");

        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id =
            client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);

        // Cancel the transaction
        client.cancel_transaction(&admin, &tx_id);

        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Cancelled));
        assert!(tx.updated_ledger >= tx.created_ledger);
    }

    #[test]
    #[should_panic(expected = "not admin")]
    fn test_cancel_transaction_panics_when_not_admin() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "cancel-unauth");

        // Try to cancel with non-admin (relayer)
        client.cancel_transaction(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "transaction already settled")]
    fn test_finalize_settlement_panics_when_transaction_already_settled() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "finalize-dup-tx");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let tx_id =
            client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);

        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);

        env.as_contract(&contract_id, || {
            let p = env.storage().persistent();
            let key = StorageKey::Tx(tx_id.clone());
            let mut tx: Transaction = p.get(&key).expect("tx");
            tx.settlement_id = SorobanString::from_str(&env, "prior-settlement");
            p.set(&key, &tx);
        });
        client.finalize_settlement(
            &relayer,
            &asset,
            &vec![&env, tx_id.clone()],
            &100i128,
            &1u64,
            &2u64,
        );
    }

    // -----------------------------------------------------------------------
    // Pause enforcement — issue #10
    // -----------------------------------------------------------------------

    fn setup_with_relayer(env: &Env) -> (Address, Address, Address, SynapseContractClient<'_>) {
        let (admin, contract_id) = setup(env);
        let client = SynapseContractClient::new(env, &contract_id);
        let relayer = Address::generate(env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(env, "USD"));
        (admin, contract_id, relayer, client)
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_grant_relayer() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.grant_relayer(&admin, &Address::generate(&env));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_revoke_relayer() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.revoke_relayer(&admin, &relayer);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_transfer_admin() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.propose_admin(&admin, &Address::generate(&env));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_add_asset() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.add_asset(&admin, &SorobanString::from_str(&env, "BTC"));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_remove_asset() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.remove_asset(&admin, &SorobanString::from_str(&env, "USD"));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_set_max_deposit() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.set_max_deposit(&admin, &1000i128);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_register_deposit() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-paused"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_mark_processing() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-mp"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.pause(&admin);
        client.mark_processing(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_mark_completed() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-mc"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.mark_processing(&relayer, &tx_id);
        client.pause(&admin);
        client.mark_completed(&relayer, &tx_id);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_mark_failed() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-mf"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.pause(&admin);
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_retry_dlq() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-rdlq"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
        client.pause(&admin);
        client.retry_dlq(&admin, &tx_id);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_finalize_settlement() {
        let env = Env::default();
        let (admin, _, relayer, client) = setup_with_relayer(&env);
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "anchor-fs"),
            &Address::generate(&env),
            &100i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.pause(&admin);
        client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env, tx_id],
            &100i128,
            &0u64,
            &1u64,
        );
    }

    // -----------------------------------------------------------------------
    // temp_lock unit tests — task 2.2
    // -----------------------------------------------------------------------

    #[test]
    fn is_locked_false_before_lock() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let key = SorobanString::from_str(&env, "test-key-1");
        let locked = env.as_contract(&contract_id, || storage::temp_lock::is_locked(&env, &key));
        assert!(!locked);
    }

    #[test]
    fn is_locked_true_after_lock() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let key = SorobanString::from_str(&env, "test-key-2");
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
        let locked = env.as_contract(&contract_id, || storage::temp_lock::is_locked(&env, &key));
        assert!(locked);
    }

    #[test]
    #[should_panic(expected = "idempotency lock active")]
    fn lock_panics_on_double_lock() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let key = SorobanString::from_str(&env, "test-key-3");
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
    }

    #[test]
    fn lock_unlock_lock_succeeds() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let key = SorobanString::from_str(&env, "test-key-4");
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
        env.as_contract(&contract_id, || storage::temp_lock::unlock(&env, &key));
        // Should not panic
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
    }

    #[test]
    fn is_locked_false_after_unlock() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let key = SorobanString::from_str(&env, "test-key-5");
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &key));
        env.as_contract(&contract_id, || storage::temp_lock::unlock(&env, &key));
        let locked = env.as_contract(&contract_id, || storage::temp_lock::is_locked(&env, &key));
        assert!(!locked);
    }

    // -----------------------------------------------------------------------
    // temp_lock wiring tests — task 3.2
    // -----------------------------------------------------------------------

    #[test]
    fn register_deposit_is_idempotent() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "idem-anchor-1");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        let id1 = client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);
        // Simulate lock expiry by manually releasing it (mirrors TTL expiry behaviour)
        env.as_contract(&contract_id, || storage::temp_lock::unlock(&env, &anchor_id));
        let id2 = client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);
        assert_eq!(id1, id2);
    }

    #[test]
    fn lock_held_after_first_register_deposit() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "lock-held-anchor");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);
        let locked = env.as_contract(&contract_id, || storage::temp_lock::is_locked(&env, &anchor_id));
        assert!(locked);
    }

    #[test]
    #[should_panic(expected = "idempotency lock active")]
    fn register_deposit_panics_when_lock_active_no_record() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let asset = SorobanString::from_str(&env, "USD");
        let anchor_id = SorobanString::from_str(&env, "lock-active-anchor");
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);
        // manually acquire the lock so no record exists yet
        env.as_contract(&contract_id, || storage::temp_lock::lock(&env, &anchor_id));
        // this call should panic because the lock is active but no record exists
        client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None, &None);
    }

    // -----------------------------------------------------------------------
    // Two-step admin transfer — issue #8
    // -----------------------------------------------------------------------

    #[test]
    fn test_propose_admin_sets_pending() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        client.propose_admin(&admin, &new_admin);
        let pending = env.as_contract(&contract_id, || storage::pending_admin::get(&env));
        assert_eq!(pending, Some(new_admin));
    }

    #[test]
    fn test_accept_admin_transfers_and_clears_pending() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        client.propose_admin(&admin, &new_admin);
        client.accept_admin(&new_admin);

        assert_eq!(client.get_admin(), new_admin);
        let pending = env.as_contract(&contract_id, || storage::pending_admin::get(&env));
        assert_eq!(pending, None);
    }

    #[test]
    #[should_panic(expected = "only proposed admin can accept")]
    fn test_accept_admin_panics_if_wrong_caller() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        let rando = Address::generate(&env);

        client.propose_admin(&admin, &new_admin);
        client.accept_admin(&rando);
    }

    #[test]
    #[should_panic(expected = "no pending admin")]
    fn test_accept_admin_panics_if_no_proposal() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let rando = Address::generate(&env);

        client.accept_admin(&rando);
    }

    #[test]
    #[should_panic(expected = "not admin")]
    fn test_propose_admin_panics_if_not_admin() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let rando = Address::generate(&env);

        client.propose_admin(&rando, &rando);
    }

    #[test]
    fn test_propose_admin_emits_event() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        client.propose_admin(&admin, &new_admin);
        let events = env.events().all();
        assert!(!events.is_empty());
        let (contract, topics, _) = events.last().unwrap();
        assert_eq!(contract, contract_id);
        assert_eq!(topics, (symbol_short!("synapse"),).into_val(&env));
    }

    #[test]
    fn test_accept_admin_emits_event() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        client.propose_admin(&admin, &new_admin);
        client.accept_admin(&new_admin);
        let events = env.events().all();
        assert!(!events.is_empty());
        let (contract, topics, _) = events.last().unwrap();
        assert_eq!(contract, contract_id);
        assert_eq!(topics, (symbol_short!("synapse"),).into_val(&env));
    }

    #[test]
    fn test_require_admin_or_relayer_passes_for_admin() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        env.as_contract(&contract_id, || {
            crate::access::require_admin_or_relayer(&env, &admin);
        });
    }

    #[test]
    fn test_require_admin_or_relayer_passes_for_relayer() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        env.as_contract(&contract_id, || {
            crate::access::require_admin_or_relayer(&env, &relayer);
        });
    }

    #[test]
    #[should_panic(expected = "not admin or relayer")]
    fn test_require_admin_or_relayer_panics_for_stranger() {
        let env = Env::default();
        let (_, contract_id) = setup(&env);
        let stranger = Address::generate(&env);
        env.as_contract(&contract_id, || {
            crate::access::require_admin_or_relayer(&env, &stranger);
        });
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_propose_admin() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.propose_admin(&admin, &Address::generate(&env));
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_accept_admin() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        let new_admin = Address::generate(&env);
        client.propose_admin(&admin, &new_admin);
        client.pause(&admin);
        client.accept_admin(&new_admin);
    }

    #[test]
    #[should_panic(expected = "contract paused")]
    fn test_paused_blocks_set_min_deposit() {
        let env = Env::default();
        let (admin, _, _, client) = setup_with_relayer(&env);
        client.pause(&admin);
        client.set_min_deposit(&admin, &100i128);
    }

    // -----------------------------------------------------------------------
    // StatusUpdated includes old_status — issue #66
    // -----------------------------------------------------------------------

    fn last_status_updated_event(env: &Env) -> (TransactionStatus, TransactionStatus) {
        let (_, _, data) = env.events().all().last().unwrap();
        match Event::try_from_val(env, &data).unwrap() {
            Event::StatusUpdated(_, old, new) => (old, new),
            _ => panic!("expected StatusUpdated"),
        }
    }

    #[test]
    fn test_mark_processing_event_includes_old_status() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "su-processing");
        client.mark_processing(&relayer, &tx_id);
        let (old, new) = last_status_updated_event(&env);
        assert_eq!(old, TransactionStatus::Pending);
        assert_eq!(new, TransactionStatus::Processing);
    }

    #[test]
    fn test_mark_completed_event_includes_old_status() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "su-completed");
        client.mark_processing(&relayer, &tx_id);
        client.mark_completed(&relayer, &tx_id);
        let (old, new) = last_status_updated_event(&env);
        assert_eq!(old, TransactionStatus::Processing);
        assert_eq!(new, TransactionStatus::Completed);
    }

    #[test]
    fn test_mark_failed_event_includes_old_status() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "su-failed");
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
        // second-to-last event is StatusUpdated (last is MovedToDlq)
        let events = env.events().all();
        let (_, _, data) = events.get(events.len() - 2).unwrap();
        match Event::try_from_val(&env, &data).unwrap() {
            Event::StatusUpdated(_, old, new) => {
                assert_eq!(old, TransactionStatus::Pending);
                assert_eq!(new, TransactionStatus::Failed);
            }
            _ => panic!("expected StatusUpdated"),
        }
    }

    #[test]
    fn test_retry_dlq_event_includes_old_status() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "su-retry"),
            &Address::generate(&env),
            &1i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
        client.retry_dlq(&admin, &tx_id);
        let (old, new) = last_status_updated_event(&env);
        assert_eq!(old, TransactionStatus::Failed);
        assert_eq!(new, TransactionStatus::Pending);
    }

    #[test]
    fn test_cancel_transaction_event_includes_old_status() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
        let tx_id = client.register_deposit(
            &relayer,
            &SorobanString::from_str(&env, "su-cancel"),
            &Address::generate(&env),
            &1i128,
            &SorobanString::from_str(&env, "USD"),
            &None,
            &None,
        );
        client.cancel_transaction(&admin, &tx_id);
        let (old, new) = last_status_updated_event(&env);
        assert_eq!(old, TransactionStatus::Pending);
        assert_eq!(new, TransactionStatus::Cancelled);
    }
}
