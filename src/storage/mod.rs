use crate::types::{DlqEntry, Settlement, Transaction};
use soroban_sdk::{contracttype, Address, Env, String as SorobanString};

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
    TempLock(SorobanString),
}

fn extend_persistent_ttl(env: &Env, key: &StorageKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TX_TTL_THRESHOLD, TX_TTL_EXTEND_TO);
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
    pub fn set(env: &Env, pending: &Address) {
        env.storage().instance().set(&StorageKey::PendingAdmin, pending);
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
        if count(env) >= MAX_ASSETS {
            panic!("max assets reached")
        }
        env.storage()
            .instance()
            .set(&StorageKey::Asset(code.clone()), &true);
        set_count(env, count(env) + 1);
    }
    pub fn remove(env: &Env, code: &SorobanString) {
        if !is_allowed(env, code) {
            return;
        }
        env.storage().instance().remove(&StorageKey::Asset(code.clone()));
        set_count(env, count(env).saturating_sub(1));
    }
    pub fn is_allowed(env: &Env, code: &SorobanString) -> bool {
        env.storage().instance().has(&StorageKey::Asset(code.clone()))
    }
    pub fn require_allowed(env: &Env, code: &SorobanString) {
        if !is_allowed(env, code) {
            panic!("asset not allowed")
        }
    }
}

pub mod min_deposit {
    use super::*;
    pub fn set(env: &Env, amount: &i128) {
        env.storage().instance().set(&StorageKey::MinDeposit, amount);
    }
    pub fn get(env: &Env) -> Option<i128> {
        env.storage().instance().get(&StorageKey::MinDeposit)
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

pub mod deposits {
    use super::*;
    pub fn save(env: &Env, tx: &Transaction) {
        let key = StorageKey::Tx(tx.id.clone());
        env.storage().persistent().set(&key, tx);
        extend_persistent_ttl(env, &key);
    }
    pub fn get(env: &Env, id: &SorobanString) -> Transaction {
        let tx_key = StorageKey::Tx(id.clone());
        let tx = env
            .storage()
            .persistent()
            .get(&tx_key)
            .expect("tx not found");
        extend_persistent_ttl(env, &tx_key);
        tx
    }
    pub fn index_anchor_id(env: &Env, anchor_id: &SorobanString, tx_id: &SorobanString) {
        let key = StorageKey::AnchorIdx(anchor_id.clone());
        env.storage().persistent().set(&key, tx_id);
        extend_persistent_ttl(env, &key);
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
    }
    pub fn get(env: &Env, id: &SorobanString) -> Settlement {
        env.storage()
            .persistent()
            .get(&StorageKey::Settlement(id.clone()))
            .expect("settlement not found")
    }
}

pub mod dlq {
    use super::*;
    pub fn push(env: &Env, entry: &DlqEntry) {
        let key = StorageKey::Dlq(entry.tx_id.clone());
        env.storage().persistent().set(&key, entry);
        extend_persistent_ttl(env, &key);
    }
    pub fn get(env: &Env, tx_id: &SorobanString) -> Option<DlqEntry> {
        let dlq_key = StorageKey::Dlq(tx_id.clone());
        let value = env.storage().persistent().get(&dlq_key);
        if value.is_some() {
            extend_persistent_ttl(env, &dlq_key);
        }
        value
    }
    pub fn remove(env: &Env, tx_id: &SorobanString) {
        env.storage().persistent().remove(&StorageKey::Dlq(tx_id.clone()));
    }
}

pub mod temp_lock {
    use super::*;

    pub fn unlock(env: &Env, key: &SorobanString) {
        env.storage()
            .temporary()
            .remove(&StorageKey::TempLock(key.clone()));
    }
}

pub use temp_lock::unlock as unlock_temp;
