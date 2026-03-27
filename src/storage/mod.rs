use crate::types::{DlqEntry, Settlement, Transaction};
use soroban_sdk::{contracttype, Address, Env, String as SorobanString};

// TODO(#59): use temporary() storage for in-flight idempotency locks
// TODO(#60): add DlqCount key to track total DLQ entries without scanning

const TX_TTL_THRESHOLD: u32 = 17_280;
const TX_TTL_EXTEND_TO: u32 = 172_800;

fn extend_persistent_ttl(env: &Env, key: &StorageKey) {
    env.storage().persistent().extend_ttl(key, TX_TTL_THRESHOLD, TX_TTL_EXTEND_TO);
}

pub const MAX_ASSETS: u32 = 20;

#[contracttype]
pub enum StorageKey {
    Admin,
    PendingAdmin,
    Paused,
    MinDeposit,
    MaxDeposit,
    AssetCount,
    TempLock(SorobanString),
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
        env.storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("not initialised")
    }
}

pub mod pending_admin {
    use super::*;
    pub fn set(env: &Env, candidate: &Address) {
        env.storage().instance().set(&StorageKey::PendingAdmin, candidate);
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

    pub fn add(env: &Env, code: &SorobanString) {
        if is_allowed(env, code) {
            return;
        }
        env.storage().instance().set(&StorageKey::Asset(code.clone()), &true);
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
        env.storage().instance().set(&StorageKey::MaxDeposit, amount);
    }

    pub fn get(env: &Env) -> Option<i128> {
        env.storage().instance().get(&StorageKey::MaxDeposit)
    }
}

pub mod min_deposit {
    use super::*;

    pub fn set(env: &Env, amount: i128) {
        env.storage().instance().set(&StorageKey::MinDeposit, &amount);
    }

    pub fn get(env: &Env) -> Option<i128> {
        env.storage().instance().get(&StorageKey::MinDeposit)
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
        let key = StorageKey::Tx(id.clone());
        let tx = env.storage().persistent().get(&key).expect("tx not found");
        extend_persistent_ttl(env, &key);
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
        let key = StorageKey::Settlement(id.clone());
        let s = env.storage().persistent().get(&key).expect("settlement not found");
        extend_persistent_ttl(env, &key);
        s
    }
}

pub mod dlq {
    use super::*;

    pub fn push(env: &Env, entry: &DlqEntry) {
        if super::pause::is_paused(env) {
            panic!("contract paused");
        }
        let mut count: i128 = env.storage().persistent().get(&StorageKey::DlqCount(0i128)).unwrap_or(0i128);
        count += 1;
        env.storage().persistent().set(&StorageKey::DlqCount(0i128), &count);
        env.storage().persistent().set(&StorageKey::Dlq(entry.tx_id.clone()), entry);
    }

    pub fn get(env: &Env, tx_id: &SorobanString) -> Option<DlqEntry> {
        let key = StorageKey::Dlq(tx_id.clone());
        let value = env.storage().persistent().get(&key);
        if value.is_some() {
            extend_persistent_ttl(env, &key);
        }
        value
    }

    pub fn remove(env: &Env, tx_id: &SorobanString) {
        let mut count: i128 = env.storage().persistent().get(&StorageKey::DlqCount(0i128)).unwrap_or(0i128);
        count = count.saturating_sub(1);
        env.storage().persistent().set(&StorageKey::DlqCount(0i128), &count);
        env.storage().persistent().remove(&StorageKey::Dlq(tx_id.clone()));
    }

    pub fn get_count(env: &Env) -> i128 {
        env.storage().persistent().get(&StorageKey::DlqCount(0i128)).unwrap_or(0i128)
    }
}

pub mod temp_lock {
    use super::*;

    const TEMP_LOCK_THRESHOLD: u32 = 3600;
    const TEMP_LOCK_EXTEND_TO: u32 = 7200;

    pub fn lock(env: &Env, key: &SorobanString) {
        let lock_key = StorageKey::TempLock(key.clone());
        if env.storage().temporary().has(&lock_key) {
            panic!("idempotency lock active");
        }
        env.storage().temporary().set(&lock_key, &true);
        env.storage().temporary().extend_ttl(&lock_key, TEMP_LOCK_THRESHOLD, TEMP_LOCK_EXTEND_TO);
    }

    pub fn unlock(env: &Env, key: &SorobanString) {
        env.storage().temporary().remove(&StorageKey::TempLock(key.clone()));
    }

    pub fn is_locked(env: &Env, key: &SorobanString) -> bool {
        env.storage().temporary().has(&StorageKey::TempLock(key.clone()))
    }
}

pub use temp_lock::{lock as lock_temp, unlock as unlock_temp, is_locked as is_temp_locked};
