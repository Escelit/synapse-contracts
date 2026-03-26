use crate::storage::{admin, pause, relayers};
use soroban_sdk::{Address, Env};

// TODO(#63): add `require_not_paused` guard and call it in every mutating fn in lib.rs
// TODO(#64): support a pending_admin two-step transfer to prevent admin lockout

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

pub fn require_not_paused(env: &Env) {
    // TODO(#63): wire this into every state-mutating function
    if pause::is_paused(env) {
        panic!("contract paused")
    }
}

pub fn require_admin_or_original_relayer(env: &Env, caller: &Address, tx_relayer: &Address) {
    caller.require_auth();
    if caller != &admin::get(env) && caller != tx_relayer {
        panic!("not admin or original relayer")
    }
}
