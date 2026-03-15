#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, vec};
use crate::{SynapseContract, SynapseContractClient};

fn setup(env: &Env) -> (Address, SynapseContractClient) {
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

fn usd(env: &Env) -> SorobanString { SorobanString::from_str(env, "USD") }

// ---------------------------------------------------------------------------
// Init — TODO(#1), TODO(#2)
// ---------------------------------------------------------------------------

#[test]
fn initialize_sets_admin() {
    let env = Env::default();
    let (_, client) = setup(&env);
    // TODO(#41): assert client.get_admin() == admin once query is added
}

#[test]
#[should_panic]
fn initialize_twice_panics() {
    // TODO(#1): implement guard, then enable this test
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.initialize(&admin);
}

// ---------------------------------------------------------------------------
// Access control — TODO(#3)–(#8), TODO(#63)–(#65)
// ---------------------------------------------------------------------------

#[test]
fn grant_and_revoke_relayer() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    assert!(client.is_relayer(&relayer));
    client.revoke_relayer(&admin, &relayer);
    assert!(!client.is_relayer(&relayer));
}

#[test]
#[should_panic]
fn non_admin_cannot_grant_relayer() {
    let env = Env::default();
    let (_, client) = setup(&env);
    let rando = Address::generate(&env);
    client.grant_relayer(&rando, &rando);
}

#[test]
fn pause_and_unpause() {
    let env = Env::default();
    let (admin, client) = setup(&env);
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
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    client.pause(&admin);
    client.register_deposit(&relayer, &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env), &100_000_000, &usd(&env));
}

// ---------------------------------------------------------------------------
// Asset allowlist — TODO(#12)–(#14)
// ---------------------------------------------------------------------------

#[test]
fn add_and_remove_asset() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.add_asset(&admin, &usd(&env));
    assert!(client.is_asset_allowed(&usd(&env)));
    client.remove_asset(&admin, &usd(&env));
    assert!(!client.is_asset_allowed(&usd(&env)));
}

#[test]
#[should_panic]
fn register_deposit_rejects_unlisted_asset() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.register_deposit(&relayer, &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env), &100_000_000, &usd(&env));
}

// ---------------------------------------------------------------------------
// Deposit registration — TODO(#15)–(#22)
// ---------------------------------------------------------------------------

#[test]
fn register_deposit_returns_tx_id() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let anchor_id = SorobanString::from_str(&env, "anchor-001");
    let tx_id = client.register_deposit(&relayer, &anchor_id, &Address::generate(&env), &100_000_000, &usd(&env));
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 100_000_000);
}

#[test]
fn register_deposit_is_idempotent() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let anchor_id = SorobanString::from_str(&env, "anchor-001");
    let depositor = Address::generate(&env);
    let id1 = client.register_deposit(&relayer, &anchor_id, &depositor, &100_000_000, &usd(&env));
    let id2 = client.register_deposit(&relayer, &anchor_id, &depositor, &100_000_000, &usd(&env));
    assert_eq!(id1, id2);
}

#[test]
#[should_panic]
fn register_deposit_rejects_non_relayer() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.add_asset(&admin, &usd(&env));
    client.register_deposit(&admin, &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env), &100_000_000, &usd(&env));
}

// TODO(#15): test minimum amount enforcement once implemented
// TODO(#16): test maximum amount enforcement once implemented
// TODO(#17): test empty anchor_transaction_id rejection once implemented

// ---------------------------------------------------------------------------
// Transaction lifecycle — TODO(#23)–(#28)
// ---------------------------------------------------------------------------

#[test]
fn full_lifecycle_pending_to_completed() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env), &50_000_000, &usd(&env));
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    // TODO(#25): assert tx.status == Completed once status is exposed on get_transaction
}

#[test]
fn mark_failed_creates_dlq_entry() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a2"),
        &Address::generate(&env), &50_000_000, &usd(&env));
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "horizon timeout"));
    // TODO(#40): assert client.get_dlq_entry(&tx_id).error_reason == "horizon timeout"
}

// TODO(#23): test Pending→Processing guard (skip to Processing from Completed should panic)
// TODO(#25): test Processing→Completed guard
// TODO(#26): test Failed transition guard

// ---------------------------------------------------------------------------
// DLQ retry — TODO(#29)–(#32)
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "not implemented")]
fn retry_dlq_panics_until_implemented() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.retry_dlq(&admin, &SorobanString::from_str(&env, "fake-id"));
}

// TODO(#29): test retry resets status to Pending
// TODO(#30): test DLQ entry removed after retry
// TODO(#31): test DlqRetried event emitted
// TODO(#32): test max retry cap

// ---------------------------------------------------------------------------
// Settlement — TODO(#33)–(#39)
// ---------------------------------------------------------------------------

#[test]
fn finalize_settlement_stores_record() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a3"),
        &Address::generate(&env), &100_000_000, &usd(&env));
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    let s_id = client.finalize_settlement(&relayer, &usd(&env),
        &vec![&env, tx_id], &100_000_000, &0u64, &1u64);
    let s = client.get_settlement(&s_id);
    assert_eq!(s.total_amount, 100_000_000);
}

// TODO(#33): test that settling a non-Completed tx panics
// TODO(#34): test that settling an already-settled tx panics
// TODO(#36): test that mismatched total_amount panics
// TODO(#37): test that period_start > period_end panics
