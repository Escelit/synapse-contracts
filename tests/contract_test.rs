#[cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, String as SorobanString, vec};
use crate::{SynapseContract, SynapseContractClient, types::TransactionStatus};

fn setup(env: &Env) -> (Address, SynapseContractClient) {
    env.mock_all_auths();
    let id = env.register_contract(None, SynapseContract);
    let client = SynapseContractClient::new(&env, &id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    (admin, client)
}

fn usd(env: &Env) -> SorobanString { SorobanString::from_str(env, "USD") }

// ---------------------------------------------------------------------------
// Init — TODO(#1), TODO(#2)
// ---------------------------------------------------------------------------

#[test]
fn test_initialize_sets_admin() {
    let env = Env::default();
    let (_, client) = setup(&env);
    // TODO(#41): assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let env = Env::default();
    let (admin, client) = setup(&env);
    client.initialize(&admin);
}

 // ... (full content would be pasted here to make it complete)

