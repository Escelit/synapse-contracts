#![no_std]

extern crate alloc;

mod access;
mod events;
mod storage;
pub mod types;

use access::{accept_pending_admin, require_admin, require_not_paused, require_relayer, set_pending_admin};
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
        let zero_addr = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if relayer == zero_addr { panic!("invalid relayer address") }
        require_admin(&env, &caller);
        relayers::add(&env, &relayer);
        emit(&env, Event::RelayerGranted(relayer));
    }

    pub fn revoke_relayer(env: Env, caller: Address, relayer: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if !relayers::has(&env, &relayer) { panic!("address is not a relayer") }
        relayers::remove(&env, &relayer);
        emit(&env, Event::RelayerRevoked(relayer));
    }

    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        let old_admin = storage::admin::get(&env);
        storage::admin::set(&env, &new_admin);
        emit(&env, Event::AdminTransferred(old_admin, new_admin));
    }

    /// Propose a new admin address for transfer.
    /// Only the current admin can propose a new admin.
    /// The proposed admin must accept the transfer to complete it.
    pub fn propose_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        set_pending_admin(&env, &new_admin);
        let current_admin = storage::admin::get(&env);
        emit(&env, Event::AdminTransferProposed(current_admin, new_admin));
    }

    /// Accept the admin transfer proposal.
    /// Only the proposed admin can accept the transfer.
    /// After acceptance, the proposed admin becomes the new admin.
    pub fn accept_admin(env: Env, caller: Address) {
        require_not_paused(&env);
        caller.require_auth();
        let old_admin = storage::admin::get(&env);
        let new_admin = accept_pending_admin(&env, &caller);
        emit(&env, Event::AdminTransferred(old_admin, new_admin));
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

    // TODO(#13): cap the total number of allowed assets to bound instance storage
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
        require_not_paused(&env);
        require_admin(&env, &caller);
        if amount <= 0 { panic!("min deposit must be positive") }
        min_deposit::set(&env, &amount);
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

    // TODO(#17): validate anchor_transaction_id is non-empty
    // TODO(#18): add `memo` field support (mirrors synapse-core CallbackPayload)
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

        let tx_id = next_id(&env, symbol_short!("txnonce"));
        let tx = Transaction::new(
            &env,
            tx_id,
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
        tx.status = TransactionStatus::Processing;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Processing));
    }

    pub fn mark_completed(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        if tx.status != TransactionStatus::Processing {
            panic!("transaction must be Processing");
        }
        tx.status = TransactionStatus::Completed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        if dlq::get(&env, &tx_id).is_some() {
            dlq::remove(&env, &tx_id);
        }
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Completed));
    }

    // TODO(#26): enforce transition guard — must be Pending or Processing
    // TODO(#28): validate error_reason is non-empty
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
        tx.status = TransactionStatus::Failed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id.clone(), TransactionStatus::Failed));
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
            panic!("not admin or original relayer")
        }

        if entry.retry_count >= MAX_RETRIES {
            emit(&env, Event::MaxRetriesExceeded(tx_id.clone()));
            panic!("max retries exceeded");
        }

        tx.status = TransactionStatus::Pending;
        tx.updated_ledger = env.ledger().sequence();
        entry.retry_count += 1;
        entry.last_retry_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        dlq::push(&env, &entry);
        dlq::remove(&env, &tx_id);

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
        settlements::save(&env, &s);

        let n = tx_ids.len();
        let mut i: u32 = 0;
        while i < n {
            let tx_id = tx_ids.get(i).unwrap();
            let mut tx = deposits::get(&env, &tx_id);
            if tx.settlement_id.len() > 0 {
                panic!("transaction already settled");
            }
            tx.settlement_id = id.clone();
            tx.updated_ledger = env.ledger().sequence();
            deposits::save(&env, &tx);
            emit(&env, Event::Settled(tx_id, id.clone()));
            i += 1;
        }

        emit(&env, Event::SettlementFinalized(id.clone(), asset_code, total_amount));
        id
    }

    pub fn get_dlq_entry(env: Env, tx_id: SorobanString) -> Option<DlqEntry> {
        dlq::get(&env, &tx_id)
    }

    pub fn get_admin(env: Env) -> Address {
        storage::admin::get(&env)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageKey;
    use crate::types::Transaction;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events as _, Ledger as _},
        vec, Env, IntoVal, String as SorobanString, TryFromVal,
    };

    fn setup(env: &Env) -> (Address, Address) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        client.initialize(&admin);
        (admin, contract_id)
    }

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

    fn event_data(env: &Env, raw: soroban_sdk::Val) -> (Event, u32) {
        <(Event, u32)>::try_from_val(env, &raw).unwrap()
    }

    #[test]
    fn test_initialize_emits_initialized_event_with_ledger() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (emitting_contract, topics, data) = events.get(0).unwrap();
        assert_eq!(emitting_contract, contract_id);
        assert_eq!(topics, (symbol_short!("synapse"),).into_val(&env));
        let (evt, ledger) = event_data(&env, data);
        assert_eq!(evt, Event::Initialized(admin));
        assert_eq!(ledger, env.ledger().sequence());
    }

    #[test]
    fn test_deposit_registered_event_includes_ledger() {
        let env = Env::default();
        env.ledger().set_sequence_number(42);
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "ledger-in-event");
        let events = env.events().all();
        let (_, _, data) = events.last().unwrap();
        let (evt, ledger) = event_data(&env, data);
        assert!(matches!(evt, Event::DepositRegistered(_, _)));
        assert_eq!(ledger, 42);
    }

    #[test]
    fn test_status_updated_event_includes_ledger() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "status-ledger");
        env.ledger().set_sequence_number(99);
        client.mark_processing(&relayer, &tx_id);
        let events = env.events().all();
        let (_, _, data) = events.last().unwrap();
        let (evt, ledger) = event_data(&env, data);
        assert!(matches!(evt, Event::StatusUpdated(_, TransactionStatus::Processing)));
        assert_eq!(ledger, 99);
    }

    #[test]
    fn test_transfer_admin_emits_event_with_ledger() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        client.transfer_admin(&admin, &new_admin);
        let events = env.events().all();
        let (_, _, data) = events.last().unwrap();
        let (evt, ledger) = event_data(&env, data);
        assert_eq!(evt, Event::AdminTransferred(admin, new_admin.clone()));
        assert_eq!(ledger, env.ledger().sequence());
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    fn test_get_admin() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
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
    #[should_panic(expected = "period_start must be <= period_end")]
    fn test_finalize_settlement_panics_when_period_start_exceeds_period_end() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
        client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env],
            &0i128,
            &2u64,
            &1u64,
        );
    }

    #[test]
    fn test_propose_admin_sets_pending() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        client.propose_admin(&admin, &new_admin);
        assert_eq!(client.get_pending_admin(), Some(new_admin));
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
        assert_eq!(client.get_pending_admin(), None);
    }
}
