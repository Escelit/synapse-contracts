use crate::types::Event;
use soroban_sdk::{symbol_short, Env};

// TODO(#66): include `old_status` in StatusUpdated event payload for full audit trail
// TODO(#67): include caller address in every event for attribution
// TODO(#68): add ledger sequence number to every event payload

pub fn emit(env: &Env, event: Event) {
    env.events().publish((symbol_short!("synapse"),), event);
}
