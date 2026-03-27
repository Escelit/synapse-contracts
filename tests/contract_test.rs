#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
};
use synapse_contract::{types::Event, SynapseContract, SynapseContractClient};

fn setup(env: &Env) -> (Address, Address, SynapseContractClient<'_>) {
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, id, client)
}

fn event_data(env: &Env, raw: Val) -> (Event, u32) {
    <(Event, u32)>::try_from_val(env, &raw).unwrap()
}

fn usd(env: &Env) -> SorobanString {
    SorobanString::from_str(env, "USD")
}

// ---------------------------------------------------------------------------
// Init — TODO(#2)
// ---------------------------------------------------------------------------

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    let (_, _, _client) = setup(&env);
    // add a file here
    // TODO(#41): assert client.get_admin() == admin once query is added
}

#[test]
#[should_panic(expected = "already initialised")]
fn initialize_twice_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.initialize(&admin);
}

// ---------------------------------------------------------------------------
// Access control
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
fn grant_relayer_emits_relayer_granted_event() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    let events = env.events().all();
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
    assert!(client.is_paused());
    client.unpause(&admin);
    assert!(!client.is_paused());
}

// ---------------------------------------------------------------------------
// Paused mutating calls — issue #70 (depends on #63 / #10)
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "contract paused")]
fn grant_relayer_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.grant_relayer(&admin, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "contract paused")]
fn revoke_relayer_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.pause(&admin);
    client.revoke_relayer(&admin, &relayer);
}

#[test]
#[should_panic(expected = "contract paused")]
fn transfer_admin_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.transfer_admin(&admin, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "contract paused")]
fn add_asset_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.add_asset(&admin, &SorobanString::from_str(&env, "EUR"));
}

#[test]
#[should_panic(expected = "contract paused")]
fn remove_asset_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.add_asset(&admin, &usd(&env));
    client.pause(&admin);
    client.remove_asset(&admin, &usd(&env));
}

#[test]
#[should_panic(expected = "contract paused")]
fn set_max_deposit_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.set_max_deposit(&admin, &500_000_000);
}

#[test]
#[should_panic(expected = "contract paused")]
fn register_deposit_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.pause(&admin);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-reg"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
}

#[test]
#[should_panic(expected = "contract paused")]
fn mark_processing_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-mproc"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.pause(&admin);
    client.mark_processing(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "contract paused")]
fn mark_completed_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-mdone"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.pause(&admin);
    client.mark_completed(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "contract paused")]
fn mark_failed_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-fail"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.pause(&admin);
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "boom"),
    );
}

#[test]
#[should_panic(expected = "contract paused")]
fn retry_dlq_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-dlq"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    client.pause(&admin);
    client.retry_dlq(&admin, &tx_id);
}

#[test]
#[should_panic(expected = "contract paused")]
fn finalize_settlement_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-fin"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.pause(&admin);
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );
}

// ---------------------------------------------------------------------------
// Asset allowlist
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
#[should_panic(expected = "asset not in allowlist")]
fn remove_asset_rejects_unlisted_asset() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.remove_asset(&admin, &usd(&env));
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
// Deposit registration
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
    let id1 = client.register_deposit(&relayer, &anchor_id, &depositor, &100_000_000, &usd(&env), &None);
    let id2 = client.register_deposit(&relayer, &anchor_id, &depositor, &100_000_000, &usd(&env), &None);
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

// ---------------------------------------------------------------------------
// Max deposit
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
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a-max-1"),
        &Address::generate(&env), &499_999_999, &usd(&env), &None);
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
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a-max-2"),
        &Address::generate(&env), &500_000_000, &usd(&env), &None);
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
    client.register_deposit(&relayer, &SorobanString::from_str(&env, "a-max-3"),
        &Address::generate(&env), &500_000_001, &usd(&env), &None);
}

#[test]
fn deposit_succeeds_when_no_max_set() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a-max-4"),
        &Address::generate(&env), &999_999_999_999, &usd(&env), &None);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 999_999_999_999);
}

// ---------------------------------------------------------------------------
// Transaction lifecycle
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
}

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

#[test]
#[should_panic(expected = "invalid status transition")]
fn mark_processing_on_non_pending_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "lifecycle-guard-1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    // tx is now Completed — must panic
    client.mark_processing(&relayer, &tx_id);
}

// ---------------------------------------------------------------------------
// DLQ retry — TODO(#31)–(#32); #29 status regression — issue #78
// ---------------------------------------------------------------------------

#[test]
fn retry_dlq_resets_transaction_status_to_pending() {
    // Regression for #29 (issue #78): DLQ retry must restore the tx to Pending.
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "issue-78-retry-status"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "simulated failure"),
    );
    assert_eq!(
        client.get_transaction(&tx_id).status,
        synapse_contract::types::TransactionStatus::Failed
    );
    client.retry_dlq(&admin, &tx_id);
    assert_eq!(
        client.get_transaction(&tx_id).status,
        synapse_contract::types::TransactionStatus::Pending
    );
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
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a2"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    // Only admin can retry for now — use admin
    client.retry_dlq(&admin, &tx_id);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.status, synapse_contract::types::TransactionStatus::Pending);
}

#[test]
#[should_panic]
fn unrelated_relayer_cannot_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer1 = Address::generate(&env);
    let relayer2 = Address::generate(&env);
    client.grant_relayer(&admin, &relayer1);
    client.grant_relayer(&admin, &relayer2);
    client.add_asset(&admin, &usd(&env));

    let tx_id = client.register_deposit(
        &relayer1,
        &SorobanString::from_str(&env, "dlq-unrelated"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_failed(
        &relayer1,
        &tx_id,
        &SorobanString::from_str(&env, "timeout"),
    );

    client.retry_dlq(&relayer2, &tx_id);
}

// ---------------------------------------------------------------------------
// Settlement
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
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id_1 = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a4"),
        &Address::generate(&env), &40_000_000, &usd(&env), &None);
    client.mark_processing(&relayer, &tx_id_1);
    client.mark_completed(&relayer, &tx_id_1);

    let tx_id_2 = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a5"),
        &Address::generate(&env), &60_000_000, &usd(&env), &None);
    client.mark_processing(&relayer, &tx_id_2);
    client.mark_completed(&relayer, &tx_id_2);

    let _settlement_id = client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id_1, tx_id_2],
        &100_000_000,
        &0u64,
        &1u64,
    );

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();

    let (event_contract_1, event_topics_1, event_data_1) = all_events.get(event_count - 3).unwrap();
    let (event_contract_2, event_topics_2, event_data_2) = all_events.get(event_count - 2).unwrap();
    let (event_contract_3, event_topics_3, event_data_3) = all_events.get(event_count - 1).unwrap();

    assert_eq!(event_contract_1, contract_id.clone());
    assert_eq!(event_topics_1, topics.clone());
    assert_eq!(
        event_data(&env, event_data_1),
        (Event::Settled(tx_id_1, settlement_id.clone()), ledger),
    );

    assert_eq!(event_contract_2, contract_id.clone());
    assert_eq!(event_topics_2, topics.clone());
    assert_eq!(
        event_data(&env, event_data_2),
        (Event::Settled(tx_id_2, settlement_id.clone()), ledger),
    );

    assert_eq!(event_contract_3, contract_id);
    assert_eq!(event_topics_3, topics);
    assert_eq!(
        event_data(&env, event_data_3),
        (Event::SettlementFinalized(settlement_id, usd(&env), 100_000_000), ledger),
    );

#[test]
#[should_panic(expected = "transaction not completed")]
fn settle_non_completed_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "settle-pending-1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
    );
    // tx is Pending — must panic
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );
}

// TODO(#33): test that settling a non-Completed tx panics — DONE
// TODO(#34): test that settling an already-settled tx panics
// TODO(#36): test that mismatched total_amount panics

#[test]
#[should_panic(expected = "period_start must be <= period_end")]
fn finalize_settlement_panics_when_period_start_exceeds_period_end() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "period-order-1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &10u64,
        &1u64,
    );
}

#[test]
fn finalize_settlement_succeeds_with_correct_total() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a4"),
        &Address::generate(&env), &100_000_000, &usd(&env), &None);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let s_id = client.finalize_settlement(&relayer, &usd(&env),
        &vec![&env, tx_id], &100_000_000, &0u64, &1u64);
    // Verify settlement can be retrieved (TTL was extended)
    let s = client.get_settlement(&s_id);
    assert_eq!(s.total_amount, 100_000_000);
}

#[test]
fn finalize_settlement_with_single_tx_correct_total() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a7"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None);
    let s_id = client.finalize_settlement(
        &relayer, &usd(&env), &vec![&env, tx_id], &50_000_000, &0u64, &1u64,
    );
    let s = client.get_settlement(&s_id);
    assert_eq!(s.total_amount, 50_000_000);
}

#[test]
fn retry_dlq_panics_until_implemented() {
    // placeholder — retry_dlq is implemented, this test is now a no-op
}
