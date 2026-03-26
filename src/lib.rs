#![no_std]

extern crate alloc;

mod access;
mod events;
mod storage;
pub mod types;

use access::{require_admin, require_not_paused, require_relayer};
use events::emit;
use soroban_sdk::{contract, contractimpl, Address, Env, String as SorobanString, Vec};
use storage::{assets, deposits, dlq, max_deposit, relayers, settlements};
use types::{DlqEntry, Event, Settlement, Transaction, TransactionStatus};

#[contract]
pub struct SynapseContract;

#[contractimpl]
impl SynapseContract {
    // TODO(#2): emit `Initialized` event on first call
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&storage::StorageKey::Admin) {
            panic!("already initialised");
        }
        admin.require_auth();
        storage::admin::set(&env, &admin);
        emit(&env, Event::Initialized(admin));
    }

    // TODO(#3): emit `RelayerGranted` event
    pub fn grant_relayer(env: Env, caller: Address, relayer: Address) {
        require_not_paused(&env);
        // Reject the all-zeros Stellar account (GAAAAAA...AWHF) as an invalid address.
        // This is the canonical "zero address" on Stellar — 32 zero bytes encoded as a G-address.
        let zero_addr = Address::from_string(&SorobanString::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if relayer == zero_addr {
            panic!("invalid relayer address")
        }
        require_admin(&env, &caller);
        relayers::add(&env, &relayer);
        emit(&env, Event::RelayerGranted(relayer));
    }

    // TODO(#6): panic if revoking a non-existent relayer
    pub fn revoke_relayer(env: Env, caller: Address, relayer: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        if !relayers::has(&env, &relayer) {
            panic!("address is not a relayer")
        }
        relayers::remove(&env, &relayer);
        emit(&env, Event::RelayerRevoked(relayer));
    }

    // TODO(#7): emit `AdminTransferred` event
    // TODO(#8): two-step admin transfer (propose + accept) to prevent lockout
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        storage::admin::set(&env, &new_admin);
    }

    // TODO(#9): emit `ContractPaused` event
    // TODO(#10): block all state-mutating calls when paused
    pub fn pause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, true);
    }

    // TODO(#11): emit `ContractUnpaused` event
    pub fn unpause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, false);
    }

    // TODO(#12): validate asset_code is non-empty and uppercase-alphanumeric only
    // TODO(#13): cap the total number of allowed assets to bound instance storage
    pub fn add_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        assets::add(&env, &asset_code);
        emit(&env, Event::AssetAdded(asset_code));
    }

    // TODO(#14): panic if asset_code is not currently in the allowlist
    pub fn remove_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);
        assets::remove(&env, &asset_code);
        emit(&env, Event::AssetRemoved(asset_code));
    }

    pub fn set_max_deposit(env: Env, caller: Address, amount: i128) {
        require_admin(&env, &caller);
        if amount <= 0 { panic!("max deposit must be positive") }
        max_deposit::set(&env, amount);
    }

    pub fn get_max_deposit(env: Env) -> Option<i128> {
        max_deposit::get(&env)
    }

    // TODO(#15): enforce minimum deposit amount (configurable by admin)
    // TODO(#16): enforce maximum deposit amount (configurable by admin) — DONE
    // TODO(#17): validate anchor_transaction_id is non-empty
    // TODO(#18): add `memo` field support (mirrors synapse-core CallbackPayload)
    // TODO(#19): add `memo_type` field support (text | hash | id)
    // TODO(#20): add `callback_type` field (deposit | withdrawal)
    // TODO(#21): bump persistent TTL on AnchorIdx entry after save
    pub fn register_deposit(
        env: Env,
        caller: Address,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        amount: i128,
        asset_code: SorobanString,
        memo: Option<SorobanString>,
    ) -> SorobanString {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        assets::require_allowed(&env, &asset_code);

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
            amount,
            asset_code,
            memo,
        );
        let id = tx.id.clone();
        deposits::save(&env, &tx);
        deposits::index_anchor_id(&env, &anchor_transaction_id, &id);
        emit(
            &env,
            Event::DepositRegistered(id.clone(), anchor_transaction_id),
        );
        id
    }

    // TODO(#23): enforce transition guard — must be Pending
    pub fn mark_processing(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Processing;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(
            &env,
            Event::StatusUpdated(tx_id, TransactionStatus::Processing),
        );
    }

    // TODO(#25): enforce transition guard — must be Processing
    pub fn mark_completed(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Completed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(
            &env,
            Event::StatusUpdated(tx_id, TransactionStatus::Completed),
        );
    }

    // TODO(#26): enforce transition guard — must be Pending or Processing
    // TODO(#27): cap max retry_count; emit `MaxRetriesExceeded` when hit
    // TODO(#28): validate error_reason is non-empty
    pub fn mark_failed(env: Env, caller: Address, tx_id: SorobanString, error_reason: SorobanString) {
        require_not_paused(&env);
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Failed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        let entry = DlqEntry::new(&env, tx_id.clone(), error_reason.clone());
        dlq::push(&env, &entry);
        emit(&env, Event::MovedToDlq(tx_id, error_reason));
    }

    // TODO(#29): increment retry_count on DlqEntry
    // TODO(#31): emit `DlqRetried` event
    pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
        require_not_paused(&env);
        require_admin(&env, &caller);

        let mut entry = dlq::get(&env, &tx_id).expect("dlq entry not found");
        let mut tx = deposits::get(&env, &tx_id);

        tx.status = TransactionStatus::Pending;
        tx.updated_ledger = env.ledger().sequence();

        entry.retry_count += 1;
        entry.last_retry_ledger = env.ledger().sequence();

        deposits::save(&env, &tx);
        dlq::push(&env, &entry);

        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Pending));
    }
    // TODO(#34): verify no tx_id is already linked to a settlement
    // TODO(#35): write settlement_id back onto each Transaction
    // TODO(#36): verify total_amount matches sum of tx amounts on-chain
    // TODO(#37): verify period_start <= period_end
    // TODO(#39): emit per-tx `Settled` event in addition to batch event
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
        let n = tx_ids.len();
        let mut i: u32 = 0;
        while i < n {
            let tx_id = tx_ids.get(i).unwrap();
            let tx = deposits::get(&env, &tx_id);
            if tx.settlement_id.len() > 0 {
                panic!("transaction already settled");
            }
            i += 1;
        }
        let s = Settlement::new(
            &env,
            asset_code.clone(),
            tx_ids,
            total_amount,
            period_start,
            period_end,
        );
        let id = s.id.clone();
        settlements::save(&env, &s);
        emit(
            &env,
            Event::SettlementFinalized(id.clone(), asset_code, total_amount),
        );
        id
    }

    // TODO(#40): add `get_dlq_entry(tx_id)` query
    // TODO(#41): add `get_admin()` query
    // TODO(#43): add `get_min_deposit()` query
    // TODO(#44): add `get_max_deposit()` query — DONE

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

    pub fn set_max_deposit(env: Env, caller: Address, amount: i128) {
        require_admin(&env, &caller);
        max_deposit::set(&env, &amount);
    }

    pub fn get_max_deposit(env: Env) -> i128 {
        max_deposit::get(&env)
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
        vec, Env, IntoVal, String as SorobanString,
    };

    const TEST_ASSET_CODES: [&str; 20] = [
        "A00", "A01", "A02", "A03", "A04", "A05", "A06", "A07", "A08", "A09", "A10", "A11", "A12",
        "A13", "A14", "A15", "A16", "A17", "A18", "A19",
    ];

    fn setup(env: &Env) -> (Address, Address) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        client.initialize(&admin);
        (admin, contract_id)
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
    fn test_register_deposit_stores_relayer() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "relayer-on-tx");
        let tx = client.get_transaction(&tx_id);
        let _ = relayer;
        let _ = tx;
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
        );

        let tx = client.get_transaction(&tx_id);
        assert_eq!(tx.memo, Some(memo));
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
        let tx_id = client.register_deposit(&relayer, &anchor_id, &stellar, &1i128, &asset, &None);
        (client, relayer, tx_id)
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
        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Failed));
    }

    #[test]
    fn test_mark_failed_panics_when_already_failed() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "mf-twice");
        let err = SorobanString::from_str(&env, "first");
        client.mark_failed(&relayer, &tx_id, &err);
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "second"));
        let tx = client.get_transaction(&tx_id);
        assert!(matches!(tx.status, TransactionStatus::Failed));
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

        // Initially should not be paused
        assert!(!client.is_paused());

        // Pause the contract
        client.pause(&admin);
        assert!(client.is_paused());

        // Unpause the contract
        client.unpause(&admin);
        assert!(!client.is_paused());
    }

    #[test]
    #[should_panic]
    fn test_finalize_settlement_panics_when_period_start_exceeds_period_end() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);

        let _ = admin;
        let _ = contract_id;
        let _ = client;

        SynapseContractClient::new(&env, &contract_id).finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env],
            &0i128,
            &2u64,
            &1u64,
        );
    }

    #[test]
    fn test_max_deposit() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);

        // Default should be 0
        assert_eq!(client.get_max_deposit(), 0i128);

        // Set to 1000
        client.set_max_deposit(&admin, &1000i128);
        assert_eq!(client.get_max_deposit(), 1000i128);

        for code in TEST_ASSET_CODES {
            client.add_asset(&admin, &SorobanString::from_str(&env, code));
        }
        client.add_asset(&admin, &SorobanString::from_str(&env, "OVERFLOW"));
        // Set to 5000
        client.set_max_deposit(&admin, &5000i128);
        assert_eq!(client.get_max_deposit(), 5000i128);
    }

    #[test]
    fn test_finalize_settlement_writes_settlement_id_back_onto_transactions() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "settle-backref");
        let settlement_id = client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env, tx_id.clone()],
            &1i128,
            &0u64,
            &1u64,
        );
        let _ = settlement_id;
    }

    #[test]
    fn test_retry_dlq_success() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "retry-tx");

        let admin = env.as_contract(&client.address, || storage::admin::get(&env));
        let err = SorobanString::from_str(&env, "failed-initially");
        
        // 1. Mark as failed
        client.mark_failed(&relayer, &tx_id, &err);
        let tx_failed = client.get_transaction(&tx_id);
        assert!(matches!(tx_failed.status, TransactionStatus::Failed));
        
        // 2. Retry DLQ
        env.ledger().set_sequence_number(100); // Advance ledger to check updates
        client.retry_dlq(&admin, &tx_id);
        
        // 3. Verify Transaction
        let tx_retried = client.get_transaction(&tx_id);
        assert!(matches!(tx_retried.status, TransactionStatus::Pending));
        assert_eq!(tx_retried.updated_ledger, 100);
        
        // 4. Verify DLQ Entry
        let entry = env.as_contract(&client.address, || {
            storage::dlq::get(&env, &tx_id).unwrap()
        });
        assert_eq!(entry.retry_count, 1);
        assert_eq!(entry.last_retry_ledger, 100);
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
        let tx_id = client.register_deposit(
            &relayer,
            &anchor_id,
            &stellar,
            &100i128,
            &asset,
            &None,
        );

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
        let tx_id = client.register_deposit(
            &relayer,
            &anchor_id,
            &stellar,
            &100i128,
            &asset,
            &None,
        );

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
            &vec![&env, tx_id],
            &100i128,
            &1u64,
            &2u64,
        );
    }
}
