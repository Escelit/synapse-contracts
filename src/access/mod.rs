use crate::storage::{admin, pause, pending_admin, relayers};
use soroban_sdk::{Address, Env};

pub fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != admin::get(env) {
        panic!("not admin")
    }
}

pub fn require_relayer(env: &Env, caller: &Address) {
    caller.require_auth();
    if !relayers::has(env, caller) {
        panic!("not relayer")
    }
}

#[allow(dead_code)]
pub fn require_admin_or_relayer(env: &Env, caller: &Address) {
    caller.require_auth();
    if *caller != admin::get(env) && !relayers::has(env, caller) {
        panic!("not admin or relayer")
    }
}

pub fn require_not_paused(env: &Env) {
    if pause::is_paused(env) {
        panic!("contract paused")
    }
}

/// @notice Proposes a new admin. Must be called by the current admin.
/// @dev Stores candidate in pending_admin; does not transfer admin rights yet.
pub fn set_pending_admin(env: &Env, caller: &Address, candidate: &Address) {
    require_admin(env, caller);
    pending_admin::set(env, candidate);
}

/// @notice Completes the two-step admin transfer. Must be called by the pending admin.
/// @dev Clears pending_admin after promotion to prevent replay.
pub fn accept_pending_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let candidate = pending_admin::get(env).expect("no pending admin");
    if *caller != candidate {
        panic!("not pending admin")
    }
    admin::set(env, caller);
    pending_admin::clear(env);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SynapseContract, SynapseContractClient};
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup(env: &Env) -> (Address, Address, Address) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SynapseContract);
        let admin = Address::generate(env);
        SynapseContractClient::new(env, &contract_id).initialize(&admin);
        (admin, contract_id, Address::generate(env))
    }

    #[test]
    fn test_set_pending_admin_stores_candidate() {
        let env = Env::default();
        let (admin, contract_id, candidate) = setup(&env);
        env.as_contract(&contract_id, || {
            set_pending_admin(&env, &admin, &candidate);
            assert_eq!(pending_admin::get(&env), Some(candidate));
        });
    }

    #[test]
    #[should_panic(expected = "not admin")]
    fn test_set_pending_admin_panics_if_not_admin() {
        let env = Env::default();
        let (_, contract_id, stranger) = setup(&env);
        env.as_contract(&contract_id, || {
            set_pending_admin(&env, &stranger, &stranger);
        });
    }

    #[test]
    fn test_accept_pending_admin_promotes_and_clears() {
        let env = Env::default();
        let (admin, contract_id, candidate) = setup(&env);
        env.as_contract(&contract_id, || {
            set_pending_admin(&env, &admin, &candidate);
            accept_pending_admin(&env, &candidate);
            assert_eq!(admin::get(&env), candidate);
            assert_eq!(pending_admin::get(&env), None);
        });
    }

    #[test]
    #[should_panic(expected = "not pending admin")]
    fn test_accept_pending_admin_panics_if_wrong_caller() {
        let env = Env::default();
        let (admin, contract_id, candidate) = setup(&env);
        let stranger = Address::generate(&env);
        env.as_contract(&contract_id, || {
            set_pending_admin(&env, &admin, &candidate);
            accept_pending_admin(&env, &stranger);
        });
    }

    #[test]
    #[should_panic(expected = "no pending admin")]
    fn test_accept_pending_admin_panics_if_no_proposal() {
        let env = Env::default();
        let (_, contract_id, stranger) = setup(&env);
        env.as_contract(&contract_id, || {
            accept_pending_admin(&env, &stranger);
        });
    }
}
