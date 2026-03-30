#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events as _},
    Address, Env, IntoVal, String as SorobanString, TryFromVal, Val,
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

/// Regression test for #91 — DepositRegistered event is emitted on register_deposit.
#[test]
fn deposit_registered_event_is_emitted() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));

    let anchor_id = SorobanString::from_str(&env, "anchor-132");
    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &Address::generate(&env),
        &100_000_000,
        &SorobanString::from_str(&env, "USD"),
        &None,
        &None,
    );

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();

    let found = all_events.iter().any(|(contract, event_topics, raw)| {
        contract == contract_id
            && event_topics == topics
            && matches!(
                <(Event, u32)>::try_from_val(&env, &raw),
                Ok((Event::DepositRegistered(ref id, ref anchor), _))
                    if *id == tx_id && *anchor == anchor_id
            )
    });

    assert!(found, "DepositRegistered event not emitted");
}
