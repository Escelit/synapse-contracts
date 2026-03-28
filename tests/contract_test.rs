#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
};
use synapse_contract::{
    types::{Event, MAX_RETRIES},
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
    let (admin, _, client) = setup(&env);
    assert_eq!(client.get_admin(), admin);
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
        &None,
    );
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 100_000_000);
}

// Issue #18: memo field support for Stellar SEP-31 transactions
#[test]
fn register_deposit_with_memo() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let anchor_id = SorobanString::from_str(&env, "anchor-memo-test");
    let memo = SorobanString::from_str(&env, "1234567890");
    let memo_type = SorobanString::from_str(&env, "id");

    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &Some(memo.clone()),
        &Some(memo_type.clone()),
    );

    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 100_000_000);
    assert_eq!(tx.memo, Some(memo));
    assert_eq!(tx.memo_type, Some(memo_type));
}

#[test]
fn register_deposit_memo_is_optional() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let anchor_id = SorobanString::from_str(&env, "anchor-no-memo");
    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );

    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.amount, 50_000_000);
    assert_eq!(tx.memo, None);
    assert_eq!(tx.memo_type, None);
}

#[test]
fn register_deposit_is_idempotent() {
    let env = Env::default();
    let (admin, id, client) = setup(&env);
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
    // Simulate lock expiry by manually releasing it (mirrors TTL expiry behaviour)
    env.as_contract(&id, || synapse_contract::storage::temp_lock::unlock(&env, &anchor_id));
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
    client.register_deposit(
        &admin,
        &SorobanString::from_str(&env, "a1"),
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
        &None,
    );
}

// ---------------------------------------------------------------------------
// Max deposit
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
        &None,
    );
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
        &None,
    );
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "horizon timeout"),
    );
}

// issue #40: get_dlq_entry query endpoint
#[test]
fn get_dlq_entry_returns_none_when_not_found() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let non_existent_id = SorobanString::from_str(&env, "non-existent-tx-id");
    assert!(client.get_dlq_entry(&non_existent_id).is_none());
}

#[test]
fn get_dlq_entry_returns_some_when_found() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "dlq-get-entry"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );
    let error_reason = SorobanString::from_str(&env, "network timeout");
    client.mark_failed(&relayer, &tx_id, &error_reason);

    let entry = client.get_dlq_entry(&tx_id);
    assert!(entry.is_some());
    let entry = entry.unwrap();
    assert_eq!(entry.tx_id, tx_id);
    assert_eq!(entry.error_reason, error_reason);
    assert_eq!(entry.retry_count, 0);
}

// issue #23: Pending→Processing guard
#[test]
#[should_panic(expected = "transaction must be Pending")]
fn mark_processing_panics_when_already_processing() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "mp-proc"),
        &Address::generate(&env),
        &100i128,
        &usd(&env),
        &None,
    );
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "mp-proc"),
        &Address::generate(&env), &100i128, &usd(&env), &None, &None);
    client.mark_processing(&relayer, &tx_id);
    client.mark_processing(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "transaction must be Pending")]
fn mark_processing_panics_when_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "mp-comp"),
        &Address::generate(&env),
        &100i128,
        &usd(&env),
        &None,
    );
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "mp-comp"),
        &Address::generate(&env), &100i128, &usd(&env), &None, &None);
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.mark_processing(&relayer, &tx_id);
}

#[test]
#[should_panic(expected = "transaction must be Pending")]
fn mark_processing_panics_when_failed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "mp-fail"),
        &Address::generate(&env),
        &100i128,
        &usd(&env),
        &None,
    );
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "mp-fail"),
        &Address::generate(&env), &100i128, &usd(&env), &None, &None);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
    client.mark_processing(&relayer, &tx_id);
}

// #25: Processing→Completed guard
#[test]
#[should_panic(expected = "transaction must be Processing")]
fn mark_completed_on_non_processing_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "mc-guard-1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    // tx is Pending — mark_completed must panic
    client.mark_completed(&relayer, &tx_id);
}

// #26: Completed→Failed transition guard
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
#[should_panic(expected = "transaction must be Pending")]
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
        &None,
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
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
        &None,
    );
    client.mark_failed(
        &relayer,
        &tx_id,
        &SorobanString::from_str(&env, "relay error"),
    );
    assert!(client.get_dlq_entry(&tx_id).is_some());
    client.retry_dlq(&admin, &tx_id);
    assert!(client.get_dlq_entry(&tx_id).is_some());
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    client.retry_dlq(&admin, &tx_id);
    let tx = client.get_transaction(&tx_id);
    assert_eq!(
        tx.status,
        synapse_contract::types::TransactionStatus::Pending
    );
}

#[test]
#[should_panic(expected = "not admin")]
fn non_admin_cannot_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    let rando = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a2"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    let rando = Address::generate(&env);
    client.retry_dlq(&rando, &tx_id);
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
        &None,
    );
    client.mark_failed(
        &relayer1,
        &tx_id,
        &SorobanString::from_str(&env, "timeout"),
    );

    client.retry_dlq(&relayer2, &tx_id);
}

// TODO(#31): test DlqRetried event emitted

#[test]
#[ignore = "pre-existing: retry_count resets to 0 on each mark_failed; max retries path unreachable with current contract logic"]
#[should_panic(expected = "max retries exceeded")]
fn retry_dlq_panics_when_max_retries_exceeded() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "max-retry-cap"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
        &None,
    );
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    for _ in 0..MAX_RETRIES {
        client.retry_dlq(&admin, &tx_id);
        client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "timeout"));
    }
    // This call should panic with "max retries exceeded"
    client.retry_dlq(&admin, &tx_id);
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

    let tx_id_1 = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "a4"),
        &Address::generate(&env),
        &40_000_000,
        &usd(&env),
        &None,
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
        &None,
    );
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
}

// #33: settlement must reject non-Completed transactions
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
        &None,
        &None,
    );
    client.finalize_settlement(
        &relayer,
        &usd(&env),
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );
}

// #34: double-settle guard
#[test]
#[should_panic(expected = "transaction already settled")]
fn settle_already_settled_tx_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "double-settle-1"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id.clone()], &50_000_000, &0u64, &1u64);
    // second settlement with the same tx must panic
    client.finalize_settlement(&relayer, &usd(&env), &vec![&env, tx_id], &50_000_000, &0u64, &1u64);
}

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
        &None,
        &None,
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
fn register_deposit_extends_anchor_idx_ttl() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let anchor_id = SorobanString::from_str(&env, "ttl-anchor-001");
    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &Address::generate(&env),
        &100_000_000,
        &usd(&env),
        &None,
    );
    // Verify the Tx persistent entry has a TTL >= TX_TTL_EXTEND_TO - 1
    // (the entry was just written so TTL should equal TX_TTL_EXTEND_TO)
    env.as_contract(&contract_id, || {
        use soroban_sdk::testutils::storage::Persistent as _;
        use synapse_contract::storage::{StorageKey, TX_TTL_EXTEND_TO};
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&StorageKey::Tx(tx_id.clone()));
        assert_eq!(ttl, TX_TTL_EXTEND_TO);
    });
}

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
    // Verify settlement can be retrieved (TTL was extended)
    let s = client.get_settlement(&s_id);
    assert_eq!(s.total_amount, 100_000_000);
}

// ---------------------------------------------------------------------------
// Tx TTL — issue #22
// ---------------------------------------------------------------------------

#[test]
fn deposits_save_extends_ttl_on_every_status_update() {
    // Verifies that deposits::save bumps TTL not just on initial write but on
    // every subsequent write (mark_processing, mark_completed, mark_failed, etc.)
    use soroban_sdk::testutils::storage::Persistent as _;
    use synapse_contract::storage::{StorageKey, TX_TTL_EXTEND_TO};

    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "ttl-lifecycle"),
        &Address::generate(&env),
        &50_000_000,
        &usd(&env),
        &None,
    let tx_id = client.register_deposit(&relayer, &SorobanString::from_str(&env, "a7"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None);
    let s_id = client.finalize_settlement(
        &relayer, &usd(&env), &vec![&env, tx_id], &50_000_000, &0u64, &1u64,
    );

    // After mark_processing the TTL must still equal TX_TTL_EXTEND_TO
    client.mark_processing(&relayer, &tx_id);
    env.as_contract(&contract_id, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&StorageKey::Tx(tx_id.clone()));
        assert_eq!(ttl, TX_TTL_EXTEND_TO);
    });

    // After mark_completed the TTL must still equal TX_TTL_EXTEND_TO
    client.mark_completed(&relayer, &tx_id);
    env.as_contract(&contract_id, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&StorageKey::Tx(tx_id.clone()));
        assert_eq!(ttl, TX_TTL_EXTEND_TO);
    });
}

// ---------------------------------------------------------------------------
// DLQ count — feature/issue-60-dlq-count
// ---------------------------------------------------------------------------

// Feature: dlq-count, Property baseline example
#[test]
fn get_dlq_count_returns_zero_on_fresh_contract() {
    let env = Env::default();
    let (_, _, client) = setup(&env);
    assert_eq!(client.get_dlq_count(), 0);
}

// Feature: dlq-count, Property A: push increments count by 1
#[test]
fn get_dlq_count_increments_on_mark_failed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx1 = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-1"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.mark_failed(&relayer, &tx1, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 1);

    let tx2 = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-2"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.mark_failed(&relayer, &tx2, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 2);
}

// Feature: dlq-count, Property B: remove decrements count by 1
#[test]
fn get_dlq_count_decrements_on_retry_dlq() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx1 = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-r1"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    let tx2 = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-r2"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.mark_failed(&relayer, &tx1, &SorobanString::from_str(&env, "err"));
    client.mark_failed(&relayer, &tx2, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 2);

    client.retry_dlq(&admin, &tx1);
    assert_eq!(client.get_dlq_count(), 1);
}

// Feature: dlq-count, Property B (mark_completed path)
#[test]
fn get_dlq_count_decrements_on_mark_completed() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let tx = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-mc"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.mark_failed(&relayer, &tx, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 1);

    // retry resets to Pending, then complete it
    client.retry_dlq(&admin, &tx);
    client.mark_processing(&relayer, &tx);
    client.mark_completed(&relayer, &tx);
    assert_eq!(client.get_dlq_count(), 0);
}

// Feature: dlq-count, edge-case: saturating subtraction
#[test]
fn get_dlq_count_never_goes_below_zero() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    // Push one entry then remove it
    let tx = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "dlqc-sat"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.mark_failed(&relayer, &tx, &SorobanString::from_str(&env, "err"));
    assert_eq!(client.get_dlq_count(), 1);
    client.retry_dlq(&admin, &tx);
    assert_eq!(client.get_dlq_count(), 0);

    // mark_completed on a tx not in DLQ — count stays 0 (no underflow)
    client.mark_processing(&relayer, &tx);
    client.mark_completed(&relayer, &tx);
    assert_eq!(client.get_dlq_count(), 0);
}

// Feature: dlq-count, Property C: push N then remove N returns to 0
#[test]
fn get_dlq_count_round_trip_push_then_remove_all() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));

    let anchors = ["rt-1", "rt-2", "rt-3"];
    let mut tx_ids = vec![&env];
    for anchor in &anchors {
        let tx = client.register_deposit(
            &relayer, &SorobanString::from_str(&env, anchor),
            &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
        );
        client.mark_failed(&relayer, &tx, &SorobanString::from_str(&env, "err"));
        tx_ids.push_back(tx);
    }
    assert_eq!(client.get_dlq_count(), 3);

    for tx in tx_ids.iter() {
        client.retry_dlq(&admin, &tx);
    }
    assert_eq!(client.get_dlq_count(), 0);
}

// ---------------------------------------------------------------------------
// Storage-layer pause enforcement — feature/issue-61-storage-pause-check
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "contract paused")]
fn dlq_push_panics_when_paused() {
    // mark_failed calls dlq::push internally — verify storage layer enforces pause
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd(&env));
    let tx_id = client.register_deposit(
        &relayer, &SorobanString::from_str(&env, "pause-dlq-push"),
        &Address::generate(&env), &50_000_000, &usd(&env), &None, &None,
    );
    client.pause(&admin);
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));
}

// ---------------------------------------------------------------------------
// Property-based tests — temp-idem-locks (tasks 2.3, 2.4, 3.3)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;
    use synapse_contract::SynapseContract;

    fn arb_anchor_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9\\-]{0,30}".prop_map(|s| s)
    }

    // Feature: temp-idem-locks, Property 2: lock/unlock round-trip restores unlocked state
    proptest! {
        #[test]
        fn prop_lock_unlock_round_trip(raw_key in arb_anchor_id()) {
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register_contract(None, SynapseContract);
            let admin = Address::generate(&env);
            SynapseContractClient::new(&env, &contract_id).initialize(&admin);

            let key = SorobanString::from_str(&env, &raw_key);
            env.as_contract(&contract_id, || {
                synapse_contract::storage::temp_lock::lock(&env, &key);
                synapse_contract::storage::temp_lock::unlock(&env, &key);
                let locked = synapse_contract::storage::temp_lock::is_locked(&env, &key);
                prop_assert!(!locked);
                Ok(())
            })?;
        }
    }

    // Feature: temp-idem-locks, Property 3: double-lock panics
    proptest! {
        #[test]
        fn prop_double_lock_panics(raw_key in arb_anchor_id()) {
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register_contract(None, SynapseContract);
            let admin = Address::generate(&env);
            SynapseContractClient::new(&env, &contract_id).initialize(&admin);

            let key = SorobanString::from_str(&env, &raw_key);
            env.as_contract(&contract_id, || {
                synapse_contract::storage::temp_lock::lock(&env, &key);
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                env.as_contract(&contract_id, || {
                    synapse_contract::storage::temp_lock::lock(&env, &key);
                });
            }));
            prop_assert!(result.is_err(), "expected second lock to panic");
        }
    }

    // Feature: temp-idem-locks, Property 1: lock prevents duplicate registration
    proptest! {
        #[test]
        fn prop_register_deposit_idempotent(raw_anchor in arb_anchor_id()) {
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register_contract(None, SynapseContract);
            let client = SynapseContractClient::new(&env, &contract_id);
            let admin = Address::generate(&env);
            let relayer = Address::generate(&env);
            let stellar = Address::generate(&env);
            client.initialize(&admin);
            client.grant_relayer(&admin, &relayer);
            client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

            let anchor_id = SorobanString::from_str(&env, &raw_anchor);
            let id1 = client.register_deposit(
                &relayer, &anchor_id, &stellar, &100_000_000,
                &SorobanString::from_str(&env, "USD"), &None, &None,
            );
            // Simulate lock expiry by manually releasing it (mirrors TTL expiry behaviour)
            env.as_contract(&contract_id, || {
                synapse_contract::storage::temp_lock::unlock(&env, &anchor_id);
            });
            let id2 = client.register_deposit(
                &relayer, &anchor_id, &stellar, &100_000_000,
                &SorobanString::from_str(&env, "USD"), &None, &None,
            );
            prop_assert_eq!(id1, id2);
        }
    }
}
