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

/// Regression test for #95 — MovedToDlq event is emitted when mark_failed is called.
#[test]
fn dlq_entry_added_event_is_emitted_on_mark_failed() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    let usd = SorobanString::from_str(&env, "USD");
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd);

    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "anchor-134"),
        &Address::generate(&env),
        &50_000_000,
        &usd,
        &None,
        &None,
    );

    let reason = SorobanString::from_str(&env, "timeout");
    client.mark_failed(&relayer, &tx_id, &reason);

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);

    let found = all_events.iter().any(|(contract, event_topics, raw)| {
        contract == contract_id
            && event_topics == topics
            && matches!(
                <(Event, u32)>::try_from_val(&env, &raw),
                Ok((Event::MovedToDlq(ref id, ref r), _)) if *id == tx_id && *r == reason
            )
    });

    assert!(found, "MovedToDlq event not emitted on mark_failed");
}
