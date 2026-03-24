#![no_std]

mod access;
mod events;
mod storage;
mod types;

use access::{require_admin, require_relayer};
use events::emit;
use soroban_sdk::{contract, contractimpl, Address, Env, String as SorobanString, Vec};
use storage::{assets, deposits, dlq, relayers, settlements};
use types::{DlqEntry, Event, Settlement, Transaction, TransactionStatus};

#[contract]
pub struct SynapseContract;

#[contractimpl]
impl SynapseContract {
    // TODO(#1): prevent re-initialisation — panic if admin already set
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        storage::admin::set(&env, &admin);
        emit(&env, Event::Initialized(admin));
    }

    // TODO(#3): emit `RelayerGranted` event
    pub fn grant_relayer(env: Env, caller: Address, relayer: Address) {
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
    }

    // TODO(#5): emit `RelayerRevoked` event
    // TODO(#6): panic if revoking a non-existent relayer
    pub fn revoke_relayer(env: Env, caller: Address, relayer: Address) {
        require_admin(&env, &caller);
        relayers::remove(&env, &relayer);
    }

    // TODO(#7): emit `AdminTransferred` event
    // TODO(#8): two-step admin transfer (propose + accept) to prevent lockout
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
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
    pub fn add_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_admin(&env, &caller);
        assets::add(&env, &asset_code);
        emit(&env, Event::AssetAdded(asset_code));
    }

    // TODO(#14): panic if asset_code is not currently in the allowlist
    pub fn remove_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_admin(&env, &caller);
        assets::remove(&env, &asset_code);
        emit(&env, Event::AssetRemoved(asset_code));
    }

    // TODO(#15): enforce minimum deposit amount (configurable by admin)
    // TODO(#16): enforce maximum deposit amount (configurable by admin)
    // TODO(#17): validate anchor_transaction_id is non-empty
    // TODO(#18): add `memo` field support (mirrors synapse-core CallbackPayload)
    // TODO(#19): add `memo_type` field support (text | hash | id)
    // TODO(#20): add `callback_type` field (deposit | withdrawal)
    pub fn register_deposit(
        env: Env,
        caller: Address,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        amount: i128,
        asset_code: SorobanString,
    ) -> SorobanString {
        require_relayer(&env, &caller);
        assets::require_allowed(&env, &asset_code);

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
        );
        let id = tx.id.clone();
        deposits::save(&env, &tx);
        deposits::index_anchor_id(&env, &anchor_transaction_id, &id);
        emit(&env, Event::DepositRegistered(id.clone(), anchor_transaction_id));
        id
    }

    // TODO(#23): enforce transition guard — must be Pending
    pub fn mark_processing(env: Env, caller: Address, tx_id: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Processing;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Processing));
    }

    // TODO(#25): enforce transition guard — must be Processing
    pub fn mark_completed(env: Env, caller: Address, tx_id: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Completed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Completed));
    }

    // TODO(#27): cap max retry_count; emit `MaxRetriesExceeded` when hit
    // TODO(#28): validate error_reason is non-empty
    pub fn mark_failed(env: Env, caller: Address, tx_id: SorobanString, error_reason: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        if !matches!(
            tx.status,
            TransactionStatus::Pending | TransactionStatus::Processing
        ) {
            panic!("transaction must be Pending or Processing")
        }
        tx.status = TransactionStatus::Failed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        let entry = DlqEntry::new(&env, tx_id.clone(), error_reason.clone());
        dlq::push(&env, &entry);
        emit(&env, Event::MovedToDlq(tx_id, error_reason));
    }

    // TODO(#29): implement — reset tx status to Pending, increment retry_count
    // TODO(#30): remove DLQ entry after successful retry
    // TODO(#31): emit `DlqRetried` event
    // TODO(#32): only admin OR original relayer should be able to retry
    pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
        require_admin(&env, &caller);
        let _ = (env, tx_id);
        panic!("not implemented")
    }

    // TODO(#33): verify each tx_id exists and has status Completed
    // TODO(#34): verify no tx_id is already linked to a settlement
    // TODO(#35): write settlement_id back onto each Transaction
    // TODO(#36): verify total_amount matches sum of tx amounts on-chain
    // TODO(#37): verify period_start <= period_end
    // TODO(#38): bump Settlement TTL after save
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
        require_relayer(&env, &caller);
        if period_start > period_end {
            panic!("period_start must be <= period_end")
        }
        let s = Settlement::new(&env, asset_code.clone(), tx_ids, total_amount, period_start, period_end);
        let id = s.id.clone();
        settlements::save(&env, &s);
        emit(&env, Event::SettlementFinalized(id.clone(), asset_code, total_amount));
        id
    }

    // TODO(#40): add `get_dlq_entry(tx_id)` query
    // TODO(#41): add `get_admin()` query
    // TODO(#43): add `get_min_deposit()` query
    // TODO(#44): add `get_max_deposit()` query

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
    use crate::storage::{StorageKey, MAX_ASSETS};
    use soroban_sdk::{
        testutils::{storage::Persistent, Address as _, Events as _},
        vec,
        Env, IntoVal, String as SorobanString, symbol_short,
    };

    const TEST_ASSET_CODES: [&str; MAX_ASSETS as usize] = [
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
    fn test_register_deposit_stores_relayer() {
        let env = Env::default();
        let (client, relayer, tx_id) = setup_relayer_deposit(&env, "relayer-on-tx");
        let tx = client.get_transaction(&tx_id);
        assert_eq!(tx.relayer, relayer);
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
        let tx_id = client.register_deposit(&relayer, &anchor_id, &stellar, &1i128, &asset);
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
    #[should_panic(expected = "transaction must be Pending or Processing")]
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
    fn test_register_deposit_extends_anchor_idx_ttl() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);
        let stellar = Address::generate(&env);
        let anchor_id = SorobanString::from_str(&env, "anchor-tx-ttl-1");
        let asset = SorobanString::from_str(&env, "USD");

        client.grant_relayer(&admin, &relayer);
        client.add_asset(&admin, &asset);

        let tx_id = client.register_deposit(
            &relayer,
            &anchor_id,
            &stellar,
            &100i128,
            &asset,
        );

        let anchor_key = StorageKey::AnchorIdx(anchor_id);
        let tx_key = StorageKey::Tx(tx_id);
        let (ttl_anchor, ttl_tx) = env.as_contract(&contract_id, || {
            let p = env.storage().persistent();
            (p.get_ttl(&anchor_key), p.get_ttl(&tx_key))
        });
        assert_eq!(
            ttl_anchor, ttl_tx,
            "AnchorIdx TTL should match Tx after register_deposit (both extended)"
        );
    }

    #[test]
    fn test_add_asset_respects_max_assets_cap() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);

        for code in TEST_ASSET_CODES {
            client.add_asset(&admin, &SorobanString::from_str(&env, code));
        }
        let n = env.as_contract(&contract_id, || crate::storage::assets::count(&env));
        assert_eq!(n, MAX_ASSETS);
    }

    #[test]
    #[should_panic(expected = "max assets reached")]
    fn test_add_asset_panics_when_cap_exceeded() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);

        for code in TEST_ASSET_CODES {
            client.add_asset(&admin, &SorobanString::from_str(&env, code));
        }
        client.add_asset(
            &admin,
            &SorobanString::from_str(&env, "OVERFLOW"),
        );
    }

    #[test]
    #[should_panic(expected = "period_start must be <= period_end")]
    fn test_finalize_settlement_panics_when_period_start_exceeds_period_end() {
        let env = Env::default();
        let (admin, contract_id) = setup(&env);
        let client = SynapseContractClient::new(&env, &contract_id);
        let relayer = Address::generate(&env);

        client.grant_relayer(&admin, &relayer);
        client.finalize_settlement(
            &relayer,
            &SorobanString::from_str(&env, "USD"),
            &vec![&env],
            &0i128,
            &2u64,
            &1u64,
        );
    }
}
