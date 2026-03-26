use crate::types::{DlqEntry, Settlement, Transaction};
use soroban_sdk::{contracttype, Address, Env, String as SorobanString};

// TODO(#59): use temporary() storage for in-flight idempotency locks
// TODO(#60): add DlqCount key to track total DLQ entries without scanning

const TX_TTL_THRESHOLD: u32 = 17_280;
const TX_TTL_EXTEND_TO: u32 = 172_800;

pub const MAX_ASSETS: u32 = 20;

#[contracttype]
pub enum StorageKey {
    Admin,
    PendingAdmin,
    Paused,
    MinDeposit,
    MaxDeposit,
    AssetCount,
    Relayer(Address),
    Asset(SorobanString),
    Tx(SorobanString),
    AnchorIdx(SorobanString),
    Settlement(SorobanString),
    Dlq(SorobanString),
    DlqCount(i128),
}

pub mod admin {
    use super::*;
    pub fn set(env: &Env, admin: &Address) {
        env.storage().instance().set(&StorageKey::Admin, admin);
    }
pub fn get(env: &Env) -> Address {
    let admin = env.storage()
        .instance()
        .get(&StorageKey::Admin)
        .expect("not initialised");
    extend_instance_ttl(env);
    admin
}
}

pub mod pending_admin {
    use super::*;
    pub fn set(env: &Env, pending_admin: &Address) {
        env.storage().instance().set(&StorageKey::PendingAdmin, pending_admin);
    }
    pub fn get(env: &Env) -> Option<Address> {
        env.storage().instance().get(&StorageKey::PendingAdmin)
    }
    pub fn clear(env: &Env) {
        env.storage().instance().remove(&StorageKey::PendingAdmin);
    }
}

pub mod pause {
    use super::*;
    pub fn set(env: &Env, paused: bool) {
        env.storage().instance().set(&StorageKey::Paused, &paused);
    }
    pub fn is_paused(env: &Env) -> bool {
        env.storage().instance().get(&StorageKey::Paused).unwrap_or(false)
    }
}

pub mod relayers {
    use super::*;
    pub fn add(env: &Env, r: &Address) {
        env.storage().instance().set(&StorageKey::Relayer(r.clone()), &true);
    }
    pub fn remove(env: &Env, r: &Address) {
        env.storage().instance().remove(&StorageKey::Relayer(r.clone()));
    }
    pub fn has(env: &Env, r: &Address) -> bool {
        env.storage().instance().has(&StorageKey::Relayer(r.clone()))
    }
}

pub mod assets {
    use super::*;
    use crate::storage::MAX_ASSETS;

    fn count(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&StorageKey::AssetCount)
            .unwrap_or(0u32)
    }

    fn set_count(env: &Env, n: u32) {
        env.storage().instance().set(&StorageKey::AssetCount, &n);
    }

    pub fn add(env: &Env, code: &SorobanString) {
        if is_allowed(env, code) {
            return;
        }
        env.storage()
            .instance()
            .set(&StorageKey::Asset(code.clone()), &true);
    }

    pub fn remove(env: &Env, code: &SorobanString) {
        env.storage().instance().remove(&StorageKey::Asset(code.clone()));
    }

    pub fn is_allowed(env: &Env, code: &SorobanString) -> bool {
        env.storage().instance().has(&StorageKey::Asset(code.clone()))
    }

    pub fn require_allowed(env: &Env, code: &SorobanString) {
        if !is_allowed(env, code) { panic!("asset not allowed") }
    }
}

pub mod max_deposit {
    use super::*;

    pub fn set(env: &Env, amount: &i128) {
        env.storage()
            .instance()
            .set(&StorageKey::MaxDeposit, amount);
    }

    pub fn get(env: &Env) -> Option<i128> {
        env.storage().instance().get(&StorageKey::MaxDeposit)
    }
    value
}
}

pub mod deposits {
    use super::*;
    pub fn save(env: &Env, tx: &Transaction) {
        let key = StorageKey::Tx(tx.id.clone());
        env.storage().persistent().set(&key, tx);
        env.storage().persistent().extend_ttl(&key, TX_TTL_THRESHOLD, TX_TTL_EXTEND_TO);
    }
pub fn get(env: &Env, id: &SorobanString) -> Transaction {
    let tx_key = StorageKey::Tx(id.clone());
    let tx = env.storage()
        .persistent()
        .get(&tx_key)
        .expect("tx not found");
    extend_persistent_ttl(env, &tx_key);
    tx
}
    pub fn index_anchor_id(env: &Env, anchor_id: &SorobanString, tx_id: &SorobanString) {
        env.storage().persistent().set(&StorageKey::AnchorIdx(anchor_id.clone()), tx_id);
    }
    pub fn find_by_anchor_id(env: &Env, anchor_id: &SorobanString) -> Option<SorobanString> {
        env.storage().persistent().get(&StorageKey::AnchorIdx(anchor_id.clone()))
    }
}

pub mod settlements {
    use super::*;
    pub fn save(env: &Env, s: &Settlement) {
        let key = StorageKey::Settlement(s.id.clone());
        env.storage().persistent().set(&key, s);
        env.storage().persistent().extend_ttl(&key, 535_679, 535_679);
    }
    pub fn get(env: &Env, id: &SorobanString) -> Settlement {
        env.storage()
            .persistent()
            .get(&StorageKey::Settlement(id.clone()))
            .expect("settlement not found")
    }
    pub fn extend_ttl(env: &Env, id: &SorobanString) {
        env.storage()
            .persistent()
            .extend_ttl(&StorageKey::Settlement(id.clone()), 535679, 535679);
    }
}

pub mod max_deposit {
    use super::*;
    pub fn set(env: &Env, amount: &i128) {
        env.storage()
            .instance()
            .set(&StorageKey::MaxDeposit, amount);
    }
    pub fn get(env: &Env) -> Option<i128> {
        env.storage().instance().get(&StorageKey::MaxDeposit)
    }
}



pub mod dlq {
    use super::*;
    pub fn push(env: &Env, entry: &DlqEntry) {
        let mut count: i128 = env.storage().persistent().get(&StorageKey::DlqCount(0i128)).unwrap_or(0i128);
        count += 1;
        env.storage().persistent().set(&StorageKey::DlqCount(0i128), &count);
        env.storage()
            .persistent()
            .set(&StorageKey::Dlq(entry.tx_id.clone()), entry);
    }
pub fn get(env: &Env, tx_id: &SorobanString) -> Option<DlqEntry> {
    let dlq_key = StorageKey::Dlq(tx_id.clone());
    let value = env.storage()
        .persistent()
        .get(&dlq_key);
    if let Some(_) = value.as_ref() {
        extend_persistent_ttl(env, &dlq_key);
    }
    value
}
    pub fn remove(env: &Env, tx_id: &SorobanString) {
        let mut count: i128 = env.storage().persistent().get(&StorageKey::DlqCount(0i128)).unwrap_or(0i128);
        count = count.saturating_sub(1);
        env.storage().persistent().set(&StorageKey::DlqCount(0i128), &count);
        env.storage()
            .persistent()
            .remove(&StorageKey::Dlq(tx_id.clone()));
    }
pub fn get_count(env: &Env) -> i128 {
    let count_key = StorageKey::DlqCount(0i128);
    let count = env.storage().persistent().get(&count_key).unwrap_or(0i128);
    extend_persistent_ttl(env, &count_key);
    count
}
}

pub mod limits {
    use super::*;
    pub fn set_min(env: &Env, amount: i128) {
        env.storage().instance().set(&StorageKey::MinDeposit, &amount);
    }
    pub fn get_min(env: &Env) -> i128 {
        env.storage().instance().get(&StorageKey::MinDeposit).unwrap_or(0)
    }
}
