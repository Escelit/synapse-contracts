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

/// Regression test for #92 — SettlementFinalized event is emitted on finalize_settlement.
#[test]
fn settlement_finalized_event_is_emitted() {
    let env = Env::default();
    let (admin, contract_id, client) = setup(&env);
    let relayer = Address::generate(&env);
    let usd = SorobanString::from_str(&env, "USD");
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &usd);

    let tx_id = client.register_deposit(
        &relayer,
        &SorobanString::from_str(&env, "anchor-133"),
        &Address::generate(&env),
        &100_000_000,
        &usd,
        &None,
        &None,
    );
    client.mark_processing(&relayer, &tx_id);
    client.mark_completed(&relayer, &tx_id);

    let settlement_id = client.finalize_settlement(
        &relayer,
        &usd,
        &vec![&env, tx_id],
        &100_000_000,
        &0u64,
        &1u64,
    );

    let all_events = env.events().all();
    let topics: soroban_sdk::Vec<Val> = (symbol_short!("synapse"),).into_val(&env);
    let ledger = env.ledger().sequence();

    let found = all_events.iter().any(|(contract, event_topics, raw)| {
        contract == contract_id
            && event_topics == topics
            && matches!(
                <(Event, u32)>::try_from_val(&env, &raw),
                Ok((Event::SettlementFinalized(ref sid, _, _), _)) if *sid == settlement_id
            )
    });

    assert!(found, "SettlementFinalized event not emitted");
}
