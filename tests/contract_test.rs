#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    vec, Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
};
use synapse_contract::{
    types::{Event, TransactionStatus, MAX_RETRIES},
    SynapseContract, SynapseContractClient,
};

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

fn register(
    env: &Env,
    client: &SynapseContractClient,
    relayer: &Address,
    anchor: &str,
    amount: i128,
) -> SorobanString {
    client.register_deposit(
        relayer,
        &SorobanString::from_str(env, anchor),
        &Address::generate(env),
        &amount,
        &usd(env),
        &None,
        &None,
    )
}

fn event_data(env: &Env, data: Val) -> (Event, u32) {
    <(Event, u32)>::try_from_val(env, &data).unwrap()
}

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    assert_eq!(client.get_admin(), admin);
}


#[test]
fn is_initialized_reflects_bootstrap_state() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(&env, &id);
    let admin = Address::generate(&env);

    assert!(!client.is_initialized());

    client.initialize(&admin);

    assert!(client.is_initialized());
}

#[test]
#[should_panic(expected = "already initialised")]
fn initialize_twice_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.initialize(&admin);
}

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
fn grant_relayer_emits_event() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);

    client.grant_relayer(&admin, &relayer);

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();
    let event_count = all_events.len();
    let (event_contract, event_topics, event_data_val) = all_events.get(event_count - 1).unwrap();

    assert_eq!(event_contract, contract_id);
    assert_eq!(event_topics, topics);
    assert_eq!(
        event_data(&env, event_data_val),
        (Event::RelayerGranted(relayer), ledger),
    );
}

#[test]
#[should_panic(expected = "address is already a relayer")]
fn grant_existing_relayer_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.grant_relayer(&admin, &relayer);
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
#[should_panic(expected = "address is not a relayer")]
fn revoke_non_relayer_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.revoke_relayer(&admin, &Address::generate(&env));
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

#[test]
fn pause_emits_event() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);

    client.pause(&admin);

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();
    let event_count = all_events.len();
    let (event_contract, event_topics, event_data_val) = all_events.get(event_count - 1).unwrap();

    assert_eq!(event_contract, contract_id);
    assert_eq!(event_topics, topics);
    assert_eq!(
        event_data(&env, event_data_val),
        (Event::ContractPaused(admin), ledger),
    );
}

#[test]
fn unpause_emits_event() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);

    client.pause(&admin);
    client.unpause(&admin);

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();
    let event_count = all_events.len();
    let (event_contract, event_topics, event_data_val) = all_events.get(event_count - 1).unwrap();

    assert_eq!(event_contract, contract_id);
    assert_eq!(event_topics, topics);
    assert_eq!(
        event_data(&env, event_data_val),
        (Event::ContractUnpaused(admin), ledger),
    );
}

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
fn register_deposit_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.pause(&admin);
    register(&env, &client, &relayer, "paused-reg", 100_000_000);
}

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
fn register_deposit_returns_tx_id() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "anchor-001", 100_000_000);
    assert_eq!(client.get_transaction(&tx_id).amount, 100_000_000);
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
        &None,
    );
    let id2 = client.register_deposit(
        &relayer,
        &anchor_id,
        &depositor,
        &100_000_000,
        &usd(&env),
        &None,
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
    register(&env, &client, &admin, "a1", 100);
}

#[test]
#[should_panic(expected = "anchor_transaction_id must not be empty")]
fn register_deposit_panics_on_empty_anchor_id() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, ""),
        &Address::generate(&env),
        &100,
        &usd(&env),
        &None,
        &None,
    );
}

#[test]
fn register_deposit_with_memo() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "anchor-memo"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &Some(SorobanString::from_str(&env, "1234567890")),
        &Some(SorobanString::from_str(&env, "id")),
    );

    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.memo, Some(SorobanString::from_str(&env, "1234567890")));
    assert_eq!(tx.memo_type, Some(SorobanString::from_str(&env, "id")));
}

#[test]
#[should_panic(expected = "invalid memo_type")]
fn register_deposit_rejects_invalid_memo_type() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "anchor-bad-memo-type"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
        &Some(SorobanString::from_str(&env, "bogus")),
    );
}

#[test]
fn set_and_get_max_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_max_deposit(&admin, &500_000_000);
    assert_eq!(client.get_max_deposit(), 500_000_000);
}

#[test]
fn set_and_get_min_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_min_deposit(&admin, &100);
    assert_eq!(client.get_min_deposit(), Some(100));
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
    register(&env, &client, &relayer, "a-max-3", 500_000_001);
}

#[test]
#[should_panic(expected = "amount below min deposit")]
fn deposit_below_min_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_min_deposit(&admin, &500);
    register(&env, &client, &relayer, "a-min-1", 499);
}

#[test]
fn full_lifecycle_pending_to_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "lifecycle-1", 50_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    assert_eq!(
        client.get_transaction(&tx_id).status,
        TransactionStatus::Completed
    );
}

#[test]
fn mark_failed_creates_dlq_entry() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "a2", 50_000_000);
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "horizon timeout"),
    );

    let entry = client.get_dlq_entry(&tx_id).unwrap();
    assert_eq!(entry.tx_id, tx_id);
}

#[test]
fn get_dlq_entry_returns_none_when_not_found() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert!(client
        .get_dlq_entry(&SorobanString::from_str(&env, "missing"))
        .is_none());
}

#[test]
#[should_panic(expected = "transaction must be Pending")]
fn mark_processing_panics_when_not_pending() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mp-proc", 100);
    client.mark_processing(&relayer, &tx_id);
    client.mark_processing(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "transaction must be Processing")]
fn mark_completed_panics_when_pending() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mp-comp", 100);
    client.mark_completed(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "error_reason must not be empty")]
fn mark_failed_panics_on_empty_reason() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mf-empty", 100);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, ""));
}

#[test]
#[should_panic(expected = "cannot fail completed transaction")]
fn mark_failed_panics_when_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mf-completed", 100);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "late error"),
    );
}

#[test]
#[should_panic(expected = "transaction must be Pending or Processing")]
fn mark_failed_panics_when_already_failed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mf-twice", 100);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "first"));
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "second"));
}

#[test]
fn retry_dlq_resets_to_pending_and_removes_entry() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "retry-1", 50_000_000);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "boom"));
    client.retry_dlq(&admin, &tx_id);

    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.status, TransactionStatus::Pending);
    assert_eq!(tx.retry_count, 1);
    assert!(client.get_dlq_entry(&tx_id).is_none());
}

#[test]
#[should_panic(expected = "not admin or original relayer")]
fn non_admin_cannot_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "retry-denied", 50_000_000);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "boom"));
    client.retry_dlq(&stranger, &tx_id);
}

// ---------------------------------------------------------------------------
// Settlement
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "not admin or original relayer")]
fn unrelated_relayer_cannot_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer1, "retry-unrelated", 50_000_000);
    client.mark_failed(&relayer1, &tx_id, &SorobanString::from_str(&env, "boom"));
    client.retry_dlq(&relayer2, &tx_id);
}

#[test]
#[should_panic(expected = "period_start must be <= period_end")]
fn finalize_settlement_panics_when_period_order_wrong() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "retry-max", 50_000_000);
    for _ in 0..MAX_RETRIES {
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "boom"));
        client.retry_dlq(&admin, &tx_id);
    }
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "boom"));
    client.retry_dlq(&admin, &tx_id);
}

#[test]
fn transfer_admin_updates_admin() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn propose_and_accept_admin() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&admin, &new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin.clone()));
    client.accept_admin(&new_admin);
    assert_eq!(client.get_admin(), new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

// ---------------------------------------------------------------------------
// Two-step admin transfer
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "not pending admin")]
fn wrong_caller_cannot_accept_admin() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let new_admin = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.propose_admin(&admin, &new_admin);
    client.accept_admin(&stranger);
}

#[test]
fn finalize_settlement_stores_record_and_backref() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "settle-1", 100_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);

    let settlement_id = client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id.clone()],
        &100_000_000,
        &0u64,
        &1u64,
    );
    assert_eq!(
        client.get_settlement(&settlement_id).total_amount,
        100_000_000
    );
    assert_eq!(client.get_transaction(&tx_id).settlement_id, settlement_id);
}

#[test]
fn finalize_settlement_emits_events() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id_1 = register(&env, &client, &relayer, "settle-ev-1", 40_000_000);
    let tx_id_2 = register(&env, &client, &relayer, "settle-ev-2", 60_000_000);
    client.mark_processing(&relayer, &tx_id_1);
    client.mark_completed(&relayer, &tx_id_1);
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
    let ledger = env.ledger().sequence();
    let event_count = all_events.len();

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
        (
            Event::SettlementFinalized(settlement_id, usd(&env), 100_000_000),
            ledger,
        ),
    );
}

#[test]
#[should_panic(expected = "total_amount mismatch")]
fn finalize_settlement_panics_when_total_amount_mismatch() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "mismatch-1", 100_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &999_999_999,
        &0u64,
        &1u64,
    );
}

#[test]
#[should_panic(expected = "transaction not completed")]
fn settle_non_completed_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "settle-pending-1", 100_000_000);
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );
}

#[test]
#[should_panic(expected = "transaction already settled")]
fn settle_already_settled_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "double-settle-1", 50_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id.clone()],
        &50_000_000,
        &0u64,
        &1u64,
    );
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &50_000_000,
        &0u64,
        &1u64,
    );
}

// ---------------------------------------------------------------------------
// Issue #317 — verify total_amount matches on-chain sum
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "period_start must be <= period_end")]
fn finalize_settlement_period_guard() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = register(&env, &client, &relayer, "period-order-1", 100_000_000);
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
fn get_dlq_count_returns_zero_on_fresh_contract() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert_eq!(client.get_dlq_count(), 0);
}

#[test]
fn get_dlq_count_increments_on_mark_failed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx1 = register(&env, &client, &relayer, "dlqc-1", 50_000_000);
    client.mark_failed(&relayer, &tx1, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 1);

    let tx2 = register(&env, &client, &relayer, "dlqc-2", 50_000_000);
    client.mark_failed(&relayer, &tx2, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 2);
}

#[test]
fn get_dlq_count_decrements_on_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx1 = register(&env, &client, &relayer, "dlqc-r1", 50_000_000);
    let tx2 = register(&env, &client, &relayer, "dlqc-r2", 50_000_000);
    client.mark_failed(&relayer, &tx1, &SorobanString::from_str(&env, "err"));
    client.mark_failed(&relayer, &tx2, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 2);

    client.retry_dlq(&admin, &tx1);
    assert_eq!(client.get_dlq_count(), 1);
}

// ---------------------------------------------------------------------------
// Paused mutating calls — issue #70 (remaining functions)
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "contract paused")]
fn set_min_deposit_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.set_min_deposit(&admin, &100_000_000);
}

#[test]
#[should_panic(expected = "contract paused")]
fn propose_admin_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.propose_admin(&admin, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "contract paused")]
fn accept_admin_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&admin, &new_admin);
    client.pause(&admin);
    client.accept_admin(&new_admin);
}

#[test]
#[should_panic(expected = "contract paused")]
fn cancel_transaction_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "paused-cancel"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );
    client.pause(&admin);
    client.cancel_transaction(&admin, &tx_id);
}

// ---------------------------------------------------------------------------
// cancel_admin_transfer — unit tests
// ---------------------------------------------------------------------------

#[test]
fn cancel_admin_transfer_happy_path() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let pending = Address::generate(&env);

    client.propose_admin(&admin, &pending);
    assert_eq!(client.get_pending_admin(), Some(pending.clone()));

    client.cancel_admin_transfer(&admin);

    // PendingAdmin must be cleared
    assert_eq!(client.get_pending_admin(), None);

    // The last event must be AdminTransferCancelled carrying the pending address
    let all_events = env.events().all();
    let (evt_contract, evt_topics, evt_data) = all_events.last().unwrap();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    assert_eq!(evt_contract, contract_id);
    assert_eq!(evt_topics, topics);
    let (event, _ledger) = event_data(&env, evt_data);
    assert_eq!(event, Event::AdminTransferCancelled(pending));
}

#[test]
#[should_panic(expected = "no pending admin transfer")]
fn cancel_admin_transfer_no_pending_admin_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.cancel_admin_transfer(&admin);
}

#[test]
#[should_panic(expected = "contract paused")]
fn cancel_admin_transfer_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.pause(&admin);
    client.cancel_admin_transfer(&admin);
}

#[test]
#[should_panic(expected = "not admin")]
fn cancel_admin_transfer_pending_admin_cannot_cancel() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let pending = Address::generate(&env);
    client.propose_admin(&admin, &pending);
    // pending admin (address B) tries to cancel — must be rejected
    client.cancel_admin_transfer(&pending);
}

// ---------------------------------------------------------------------------
// cancel_admin_transfer — property-based tests
// ---------------------------------------------------------------------------

use proptest::prelude::*;

// Feature: admin-transfer-cancelled-variant, Property 2: successful cancellation clears pending admin
// Validates: Requirements 2.4
proptest! {
    #[test]
    fn prop_cancel_admin_transfer_clears_pending_admin(_seed in any::<u64>()) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let pending = Address::generate(&env);
        client.propose_admin(&admin, &pending);

        client.cancel_admin_transfer(&admin);

        prop_assert_eq!(client.get_pending_admin(), None);
    }
}

// Feature: admin-transfer-cancelled-variant, Property 3: event payload matches the cancelled pending admin address
// Validates: Requirements 2.5, 4.2
proptest! {
    #[test]
    fn prop_cancel_admin_transfer_event_payload_matches_pending_admin(_seed in any::<u64>()) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let pending = Address::generate(&env);
        client.propose_admin(&admin, &pending);

        client.cancel_admin_transfer(&admin);

        let all_events = env.events().all();
        let (_, _, evt_data) = all_events.last().unwrap();
        let (event, _ledger) = event_data(&env, evt_data);
        prop_assert_eq!(event, Event::AdminTransferCancelled(pending));
    }
}

// Feature: admin-transfer-cancelled-variant, Property 4: exactly one AdminTransferCancelled event, no other admin-lifecycle events
// Validates: Requirements 4.1, 4.3
proptest! {
    #[test]
    fn prop_cancel_admin_transfer_exactly_one_cancelled_event_no_other_admin_events(_seed in any::<u64>()) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SynapseContract);
        let client = SynapseContractClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let pending = Address::generate(&env);
        client.propose_admin(&admin, &pending);

        // Snapshot event count before the cancel call so we only inspect
        // events emitted during cancel_admin_transfer itself.
        let events_before = env.events().all().len();

        client.cancel_admin_transfer(&admin);

        let all_events = env.events().all();
        let total_events = all_events.len();

        let mut cancelled_count = 0u32;
        let mut transferred_count = 0u32;
        let mut proposed_count = 0u32;

        for i in events_before..total_events {
            let (_contract, _topics, raw_data) = all_events.get(i).unwrap();
            if let Ok((event, _ledger)) = <(Event, u32)>::try_from_val(&env, &raw_data) {
                match event {
                    Event::AdminTransferCancelled(_) => cancelled_count += 1,
                    Event::AdminTransferred(_, _) => transferred_count += 1,
                    Event::AdminTransferProposed(_, _) => proposed_count += 1,
                    _ => {}
                }
            }
        }

        prop_assert_eq!(cancelled_count, 1);
        prop_assert_eq!(transferred_count, 0);
        prop_assert_eq!(proposed_count, 0);
    }
}

// ---------------------------------------------------------------------------
// Issue #98: validate amount > 0 in register_deposit
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "amount must be greater than zero")]
fn register_deposit_panics_when_amount_is_zero() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "zero-amount"),
        &Address::generate(&env),
        &0,
        &usd(&env),
        &None,
        &None,
    );
}

#[test]
#[should_panic(expected = "amount must be greater than zero")]
fn register_deposit_panics_when_amount_is_negative() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "negative-amount"),
        &Address::generate(&env),
        &-1,
        &usd(&env),
        &None,
        &None,
    );
}

// ---------------------------------------------------------------------------
// Max deposit rejection — regression for #16
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "amount exceeds max deposit")]
fn register_deposit_panics_when_amount_exceeds_max_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_max_deposit(&admin, &100_000_000);
    client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "issue-117-above-max"),
        &Address::generate(&env),
        &100_000_001,
        &usd(&env),
        &None,
        &None,
    );
}

// ---------------------------------------------------------------------------
// Cancel guard — regression for #96
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "transaction must be Pending to cancel")]
fn cancel_transaction_panics_when_processing() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "issue-114-cancel-guard"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.cancel_transaction(&admin, &tx_id);
}
