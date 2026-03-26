#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Events as _},
    symbol_short,
    vec,
    Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
};
use synapse_contract::types::Event;
use synapse_contract::{SynapseContract, SynapseContractClient};

fn setup(env: &Env) -> (Address, Address, SynapseContractClient<'_>) {
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, id, client)
}

fn usd(env: &Env) -> SorobanString {
    SorobanString::from_str(env, "USD")
}

// ---------------------------------------------------------------------------
// Init — TODO(#1), TODO(#2)
// ---------------------------------------------------------------------------

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    let (_, _, _client) = setup(&env);
    // TODO(#41): assert client.get_admin() == admin once query is added
}

#[test]
#[should_panic]
fn initialize_twice_panics() {
    // TODO(#1): implement guard, then enable this test
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.initialize(&admin);
}

// ---------------------------------------------------------------------------
// Access control — TODO(#3)–(#8), TODO(#63)–(#65)
// ---------------------------------------------------------------------------

#[test]
fn grant_and_revoke_relayer() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    assert!(client.is_relayer(&relayer));
    client.revoke_relayer(&admin, &relayer);
    assert!(!client.is_relayer(&relayer));
}

#[test]
fn revoke_relayer_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(&env, &id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.revoke_relayer(&admin, &relayer);
    let events = env.events().all();
    // The last event should be RelayerRevoked containing the revoked relayer address
    assert!(!events.is_empty());
}

#[test]
#[should_panic(expected = "not admin")]
fn non_admin_cannot_grant_relayer() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    let rando = Address::generate(&env);
    client.grant_relayer(&rando, &rando);
}

#[test]
fn pause_and_unpause() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    // TODO(#42): assert client.is_paused() == true
    client.unpause(&admin);
    // TODO(#42): assert client.is_paused() == false
}

#[test]
#[should_panic]
fn mutating_call_while_paused_panics() {
    // TODO(#63): wire require_not_paused, then enable this test
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.pause(&admin);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
}

// ---------------------------------------------------------------------------
// Asset allowlist — TODO(#12)–(#14)
// ---------------------------------------------------------------------------

#[test]
fn add_and_remove_asset() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.add_asset(&admin, &usd(&env));
    assert!(client.is_asset_allowed(&usd(&env)));
    client.remove_asset(&admin, &usd(&env));
    assert!(!client.is_asset_allowed(&usd(&env)));
}

#[test]
#[should_panic(expected = "asset not allowed")]
fn register_deposit_rejects_unlisted_asset() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
}

// ---------------------------------------------------------------------------
// Deposit registration — TODO(#15)–(#22)
// ---------------------------------------------------------------------------

#[test]
fn register_deposit_returns_tx_id() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let anchor_id = SorobanString::from_str(&env, "anchor-001");
    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 100_000_000);
}

#[test]
fn register_deposit_is_idempotent() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let anchor_id = SorobanString::from_str(&env, "anchor-001");
    let depositor = Address::generate(&env);
    let id1 = client.register_deposit(
        &relayer,
        &anchor_id,
        &depositor,
        &100_000_000,
        &usd(&env),
        &None,
    );
    let id2 = client.register_deposit(
        &relayer,
        &anchor_id,
        &depositor,
        &100_000_000,
        &usd(&env),
        &None,
    );
    assert_eq!(id1, id2);
}

#[test]
#[should_panic(expected = "not relayer")]
fn register_deposit_rejects_non_relayer() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.add_asset(&admin, &usd(&env));
    client.register_deposit(
        &admin,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
}

// TODO(#17): test empty anchor_transaction_id rejection once implemented

// ---------------------------------------------------------------------------
// Min deposit — issue #15
// ---------------------------------------------------------------------------

#[test]
fn get_min_deposit_returns_none_before_set() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert!(client.get_min_deposit().is_none());
}

#[test]
fn set_and_get_min_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_min_deposit(&admin, &100_000_000);
    assert_eq!(client.get_min_deposit(), Some(100_000_000));
}

#[test]
#[should_panic]
fn non_admin_cannot_set_min_deposit() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    let rando = Address::generate(&env);
    client.set_min_deposit(&rando, &100_000_000);
}

#[test]
#[should_panic]
fn set_min_deposit_rejects_zero() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_min_deposit(&admin, &0);
}

#[test]
#[should_panic]
fn set_min_deposit_rejects_negative() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_min_deposit(&admin, &-1);
}

#[test]
fn deposit_succeeds_when_no_min_set() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-min-0"),
        &Address::generate(&env),
        &1,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 1);
}

#[test]
fn deposit_at_min_succeeds() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_min_deposit(&admin, &100_000_000);
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-min-at"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 100_000_000);
}

#[test]
#[should_panic(expected = "amount below min deposit")]
fn deposit_below_min_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_min_deposit(&admin, &100_000_000);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-min-below"),
        &Address::generate(&env),
        &99_999_999,
        &usd(&env),
        &None,
    );
}

// ---------------------------------------------------------------------------
// Max deposit — issue #16
// ---------------------------------------------------------------------------

#[test]
fn get_max_deposit_returns_none_before_set() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert!(client.get_max_deposit().is_none());
}

#[test]
fn set_and_get_max_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_max_deposit(&admin, &500_000_000);
    assert_eq!(client.get_max_deposit(), Some(500_000_000));
}

#[test]
#[should_panic]
fn non_admin_cannot_set_max_deposit() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    let rando = Address::generate(&env);
    client.set_max_deposit(&rando, &500_000_000);
}

#[test]
#[should_panic]
fn set_max_deposit_rejects_zero() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_max_deposit(&admin, &0);
}

#[test]
#[should_panic]
fn set_max_deposit_rejects_negative() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_max_deposit(&admin, &-1);
}

#[test]
fn deposit_below_max_succeeds() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_max_deposit(&admin, &500_000_000);
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-max-1"),
        &Address::generate(&env),
        &499_999_999,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 499_999_999);
}

#[test]
fn deposit_at_max_succeeds() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_max_deposit(&admin, &500_000_000);
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-max-2"),
        &Address::generate(&env),
        &500_000_000,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 500_000_000);
}

#[test]
#[should_panic(expected = "amount exceeds max deposit")]
fn deposit_above_max_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_max_deposit(&admin, &500_000_000);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-max-3"),
        &Address::generate(&env),
        &500_000_001,
        &usd(&env),
        &None,
    );
}

#[test]
fn deposit_succeeds_when_no_max_set() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    // no set_max_deposit call — should pass any amount
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a-max-4"),
        &Address::generate(&env),
        &999_999_999_999,
        &usd(&env),
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 999_999_999_999);
}

// ---------------------------------------------------------------------------
// Transaction lifecycle — TODO(#23)–(#28)
// ---------------------------------------------------------------------------

#[test]
fn full_lifecycle_pending_to_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(
        tx.status,
        synapse_contract::types::TransactionStatus::Completed
    );
}

#[test]
fn mark_failed_creates_dlq_entry() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a2"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "horizon timeout"),
    );
    let entry = client
        .get_dlq_entry(&tx_id)
        .expect("DLQ entry should exist after mark_failed");
    assert_eq!(entry.error_reason, SorobanString::from_str(&env, "horizon timeout"));
}

// TODO(#23): test Pending→Processing guard (skip to Processing from Completed should panic)
// TODO(#25): test Processing→Completed guard

#[test]
#[should_panic(expected = "cannot fail completed transaction")]
fn mark_failed_panics_when_transaction_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "tx-fail-guard"),
        &Address::generate(&env),
        &10_000_000,
        &usd(&env),
        &None,
    );

    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(
        tx.status,
        synapse_contract::types::TransactionStatus::Completed
    );

    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "late error"),
    );
}

// ---------------------------------------------------------------------------
// DLQ retry — TODO(#29)–(#32)
// ---------------------------------------------------------------------------

#[test]
fn admin_can_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    client.retry_dlq(&admin, &tx_id);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.status, synapse_contract::types::TransactionStatus::Pending);
}

#[test]
fn dlq_entry_removed_after_successful_retry() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "dlq-remove-1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "relay error"),
    );
    assert!(client.get_dlq_entry(&tx_id).is_some());
    client.retry_dlq(&admin, &tx_id);
    assert!(client.get_dlq_entry(&tx_id).is_none());
}

#[test]
#[should_panic(expected = "not admin")]
fn non_admin_cannot_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a2"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    client.retry_dlq(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "dlq entry not found")]
fn retry_dlq_panics_when_no_dlq_entry() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.retry_dlq(&admin, &SorobanString::from_str(&env, "fake-id"));
}

// TODO(#31): test DlqRetried event emitted
// TODO(#32): test max retry cap

// ---------------------------------------------------------------------------
// Settlement — TODO(#33)–(#39)
// ---------------------------------------------------------------------------

#[test]
fn finalize_settlement_stores_record() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a3"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let s_id = client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );
    let s = client.get_settlement(&s_id);
    assert_eq!(s.total_amount, 100_000_000);
}

#[test]
fn finalize_settlement_emits_settlement_finalized_event() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id_1 = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a4"),
        &Address::generate(&env),
        &40_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id_1);
    client.mark_completed(&relayer, &tx_id_1);

    let tx_id_2 = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a5"),
        &Address::generate(&env),
        &60_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id_2);
    client.mark_completed(&relayer, &tx_id_2);

    let settlement_id = client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id_1.clone(), tx_id_2.clone()],
        &100_000_000,
        &0u64,
        &1u64,
    );

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let (emitting_contract, event_topics, event_data) =
        all_events.get(all_events.len() - 1).unwrap();

    assert_eq!(emitting_contract, contract_id);
    assert_eq!(event_topics, topics);
    assert_eq!(
        Event::try_from_val(&env, &event_data).unwrap(),
        Event::SettlementFinalized(settlement_id, usd(&env), 100_000_000),
    );
}

// TODO(#33): test that settling a non-Completed tx panics
// TODO(#34): test that settling an already-settled tx panics
// TODO(#36): test that mismatched total_amount panics
// TODO(#37): test that period_start > period_end panics

#[test]
fn finalize_settlement_extends_ttl() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a4"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let s_id = client.finalize_settlement(&relayer, &usd(&env),
        &vec![&env, tx_id], &100_000_000, &0u64, &1u64);
    // Verify settlement can be retrieved (TTL was extended)
    let s = client.get_settlement(&s_id);
    assert_eq!(s.id, s_id);
    assert_eq!(s.total_amount, 100_000_000);
}
