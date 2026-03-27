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

/// Stores candidate as pending admin (no auth check — caller must call require_admin first).
pub fn set_pending_admin(env: &Env, candidate: &Address) {
    pending_admin::set(env, candidate);
}

/// Completes the two-step admin transfer. Must be called by the pending admin.
/// Returns the new admin address.
pub fn accept_pending_admin(env: &Env, caller: &Address) -> Address {
    let candidate = pending_admin::get(env).expect("no pending admin");
    if *caller != candidate {
        panic!("not pending admin")
    }
    admin::set(env, &candidate);
    pending_admin::clear(env);
    candidate
}
