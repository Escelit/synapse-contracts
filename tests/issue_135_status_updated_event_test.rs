#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
};
use synapse_contract::{
    types::{Event, TransactionStatus},
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

fn has_status_updated(env: &Env, contract_id: &Address, tx_id: &SorobanString, status: TransactionStatus) -> bool {
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(env);
    env.events().all().iter().any(|(contract, event_topics, raw)| {
        contract == *contract_id
            && event_topics == topics
            && matches!(
                <(Event, u32)>::try_from_val(env, &raw),
                Ok((Event::StatusUpdated(ref id, ref s), _)) if id == tx_id && s == &status
            )
    })
}

fn register(env: &Env, client: &SynapseContractClient, relayer: &Address, anchor: &str) -> SorobanString {
    client.register_deposit(
        relayer,
        &SorobanString::from_str(env, anchor),
        &Address::generate(env),
        &50_000_000,
        &SorobanString::from_str(env, "USD"),
        &None,
        &None,
    )
}

/// Regression for #90 — StatusUpdated emitted on mark_processing.
#[test]
fn status_updated_emitted_on_mark_processing() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

    let tx_id = register(&env, &client, &relayer, "su-processing");
    client.mark_processing(&relayer, &tx_id);

    assert!(has_status_updated(&env, &contract_id, &tx_id, TransactionStatus::Processing));
}

/// Regression for #90 — StatusUpdated emitted on mark_completed.
#[test]
fn status_updated_emitted_on_mark_completed() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

    let tx_id = register(&env, &client, &relayer, "su-completed");
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);

    assert!(has_status_updated(&env, &contract_id, &tx_id, TransactionStatus::Completed));
}

/// Regression for #90 — StatusUpdated emitted on mark_failed.
#[test]
fn status_updated_emitted_on_mark_failed() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

    let tx_id = register(&env, &client, &relayer, "su-failed");
    client.mark_failed(&relayer, &tx_id, &SorobanString::from_str(&env, "err"));

    assert!(has_status_updated(&env, &contract_id, &tx_id, TransactionStatus::Failed));
}

/// Regression for #90 — StatusUpdated emitted on cancel_transaction.
#[test]
fn status_updated_emitted_on_cancel_transaction() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

    let tx_id = register(&env, &client, &relayer, "su-cancelled");
    client.cancel_transaction(&admin, &tx_id);

    assert!(has_status_updated(&env, &contract_id, &tx_id, TransactionStatus::Cancelled));
}
