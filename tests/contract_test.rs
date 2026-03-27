#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    vec, Address, Env, String as SorobanString,
};
use synapse_contract::{
    types::{TransactionStatus, MAX_RETRIES},
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

fn register(env: &Env, client: &SynapseContractClient, relayer: &Address, anchor: &str, amount: i128) -> SorobanString {
    client.register_deposit(relayer, &SorobanString::from_str(env, anchor), &Address::generate(env), &amount, &usd(env), &None, &None)
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "already initialised")]
fn initialize_twice_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.initialize(&admin);
}

// ---------------------------------------------------------------------------
// Relayer management
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

// ---------------------------------------------------------------------------
// Pause
// ---------------------------------------------------------------------------

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
    register(&env, &client, &relayer, "paused-reg", 100);
}

#[test]
#[should_panic(expected = "contract paused")]
fn finalize_settlement_panics_when_paused() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "paused-fin", 100);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.pause(&admin);
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id], &100, &0u64, &1u64);
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
fn remove_asset_rejects_unlisted() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.remove_asset(&admin, &usd(&env));
}

#[test]
#[should_panic(expected = "invalid asset code")]
fn add_asset_panics_on_empty_code() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.add_asset(&admin, &SorobanString::from_str(&env, ""));
}

#[test]
#[should_panic(expected = "invalid asset code")]
fn add_asset_panics_on_lowercase() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.add_asset(&admin, &SorobanString::from_str(&env, "usd"));
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
    let id1 = register(&env, &client, &relayer, "anchor-idem", 100);
    let id2 = register(&env, &client, &relayer, "anchor-idem", 100);
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
    client.register_deposit(&relayer, &SorobanString::from_str(&env, ""), &Address::generate(&env), &100, &usd(&env), &None, &None);
}

// ---------------------------------------------------------------------------
// Max / min deposit
// ---------------------------------------------------------------------------

#[test]
fn get_max_deposit_returns_zero_before_set() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert_eq!(client.get_max_deposit(), 0);
}

#[test]
fn set_and_get_max_deposit() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.set_max_deposit(&admin, &500_000_000);
    assert_eq!(client.get_max_deposit(), 500_000_000);
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
    register(&env, &client, &relayer, "over-max", 500_000_001);
}

#[test]
#[should_panic(expected = "amount below min deposit")]
fn deposit_below_min_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.set_min_deposit(&admin, &50);
    register(&env, &client, &relayer, "below-min", 10);
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
    let tx_id = register(&env, &client, &relayer, "lifecycle-1", 50_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    assert_eq!(client.get_transaction(&tx_id).status, TransactionStatus::Completed);
}

#[test]
#[should_panic(expected = "transaction must be Pending")]
fn mark_processing_panics_when_not_pending() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "mp-twice", 100);
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
    let tx_id = register(&env, &client, &relayer, "mc-pending", 100);
    client.mark_completed(&relayer, &tx_id);
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
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err1"));
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err2"));
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

// ---------------------------------------------------------------------------
// DLQ retry
// ---------------------------------------------------------------------------

#[test]
fn retry_dlq_resets_to_pending() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "retry-1", 100);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    client.retry_dlq(&admin, &tx_id);
    assert_eq!(client.get_transaction(&tx_id).status, TransactionStatus::Pending);
    // Entry stays to preserve retry_count; removed on mark_completed
    assert_eq!(client.get_dlq_entry(&tx_id).unwrap().retry_count, 1);
}

#[test]
#[should_panic(expected = "max retries exceeded")]
fn retry_dlq_panics_when_max_retries_exceeded() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "max-retry", 100);
    // fail → retry MAX_RETRIES times (each retry removes from DLQ, so re-fail each time)
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    for _ in 0..MAX_RETRIES {
        client.retry_dlq(&admin, &tx_id);
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    }
    client.retry_dlq(&admin, &tx_id);
}

#[test]
fn get_dlq_entry_returns_none_when_not_found() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert!(client.get_dlq_entry(&SorobanString::from_str(&env, "nope")).is_none());
}

// ---------------------------------------------------------------------------
// Two-step admin transfer
// ---------------------------------------------------------------------------

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

#[test]
#[should_panic(expected = "not pending admin")]
fn accept_admin_panics_if_wrong_caller() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    client.propose_admin(&admin, &Address::generate(&env));
    client.accept_admin(&Address::generate(&env));
}

#[test]
#[should_panic(expected = "no pending admin")]
fn accept_admin_panics_if_no_proposal() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    client.accept_admin(&Address::generate(&env));
}

// ---------------------------------------------------------------------------
// Settlement
// ---------------------------------------------------------------------------

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
    let s_id = client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id.clone()], &100_000_000, &0u64, &1u64);
    assert_eq!(client.get_settlement(&s_id).total_amount, 100_000_000);
    assert_eq!(client.get_transaction(&tx_id).settlement_id, s_id);
}

#[test]
#[should_panic(expected = "period_start must be <= period_end")]
fn finalize_settlement_panics_when_period_order_wrong() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env], &0, &10u64, &1u64);
}

// ---------------------------------------------------------------------------
// Issue #315 — prevent double-settle
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "transaction already settled")]
fn finalize_settlement_panics_when_transaction_already_settled() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "double-settle", 100_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    // First settlement — succeeds
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id.clone()], &100_000_000, &0u64, &1u64);
    // Second settlement with same tx — must panic
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id], &100_000_000, &0u64, &1u64);
}

#[test]
fn finalize_settlement_succeeds_when_transactions_unsettled() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = register(&env, &client, &relayer, "settle-ok", 100_000_000);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let s_id = client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id], &100_000_000, &0u64, &1u64);
    assert!(s_id.len() > 0);
}
