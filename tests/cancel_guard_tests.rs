#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString};
use synapse_contract::{SynapseContract, SynapseContractClient};

fn setup(env: &Env) -> (Address, Address, SynapseContractClient<'_>) {
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, id, client)
}

fn register_pending(
    env: &Env,
    client: &SynapseContractClient<'_>,
    admin: &Address,
    relayer: &Address,
    anchor_id: &str,
) -> SorobanString {
    client.grant_relayer(admin, relayer);
    client.add_asset(admin, &SorobanString::from_str(env, "USD"));
    client.register_deposit(
        relayer,
        &SorobanString::from_str(env, anchor_id),
        &Address::generate(env),
        &100_000_000,
        &SorobanString::from_str(env, "USD"),
        &None,
        &None,
    )
}

// ---------------------------------------------------------------------------
// Success: cancel a Pending transaction
// ---------------------------------------------------------------------------

#[test]
fn cancel_pending_transaction_succeeds() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    let tx_id = register_pending(&env, &client, &admin, &relayer, "anchor-cancel-ok");

    client.cancel_transaction(&admin, &tx_id);

    use synapse_contract::types::TransactionStatus;
    let tx = client.get_transaction(&tx_id);
    assert_eq!(tx.status, TransactionStatus::Cancelled);
}

// ---------------------------------------------------------------------------
// Failure: cancel a Processing transaction
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "only Pending transactions can be cancelled")]
fn cancel_processing_transaction_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    let tx_id = register_pending(&env, &client, &admin, &relayer, "anchor-cancel-proc");

    client.mark_processing(&relayer, &tx_id);
    client.cancel_transaction(&admin, &tx_id);
}

// ---------------------------------------------------------------------------
// Failure: cancel a Completed transaction
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "only Pending transactions can be cancelled")]
fn cancel_completed_transaction_panics() {
    let env = Env::default();
    let (admin, _, client) = setup(&env);
    let relayer = Address::generate(&env);
    let tx_id = register_pending(&env, &client, &admin, &relayer, "anchor-cancel-done");

    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);
    client.cancel_transaction(&admin, &tx_id);
}
