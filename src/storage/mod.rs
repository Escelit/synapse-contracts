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
    MaxRetries,
    AssetCount,
    Relayer(Address),
    Asset(SorobanString),
    Tx(SorobanString),
    AnchorIdx(SorobanString),
    Settlement(SorobanString),
    Dlq(SorobanString),
    DlqCount(i128),
}

fn extend_instance_ttl(env: &Env) {
    env.storage().instance().extend_ttl(17_280, 172_800);
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
        let admin = env
            .storage()
            .instance()
            .get(&StorageKey::Admin)
            .expect("not initialised");
        extend_instance_ttl(env);
        admin
    }
}

pub mod pending_admin {
    use super::*;
    pub fn set(env: &Env, candidate: &Address) {
        env.storage()
            .instance()
            .set(&StorageKey::PendingAdmin, candidate);
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
        env.storage()
            .instance()
            .get(&StorageKey::Paused)
            .unwrap_or(false)
    }
}

pub mod relayers {
    use super::*;
    pub fn add(env: &Env, r: &Address) {
        env.storage()
            .instance()
            .set(&StorageKey::Relayer(r.clone()), &true);
    }
    pub fn remove(env: &Env, r: &Address) {
        env.storage()
            .instance()
            .remove(&StorageKey::Relayer(r.clone()));
    }
    pub fn has(env: &Env, r: &Address) -> bool {
        env.storage()
            .instance()
            .has(&StorageKey::Relayer(r.clone()))
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
        let n = count(env);
        if n >= super::MAX_ASSETS {
            panic!("asset cap reached");
        }
        env.storage()
            .instance()
            .set(&StorageKey::Asset(code.clone()), &true);
        set_count(env, n + 1);
    }

    pub fn remove(env: &Env, code: &SorobanString) {
        if !is_allowed(env, code) {
            return;
        }
        env.storage()
            .instance()
            .remove(&StorageKey::Asset(code.clone()));
        let n = count(env);
        set_count(env, n.saturating_sub(1));
    }

    pub fn is_allowed(env: &Env, code: &SorobanString) -> bool {
        env.storage()
            .instance()
            .has(&StorageKey::Asset(code.clone()))
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
        env.storage()
            .instance()
            .set(&StorageKey::MinDeposit, amount);
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

pub mod max_retries {
    use super::*;
    pub fn set(env: &Env, max_retries: &u32) {
        env.storage()
            .instance()
            .set(&StorageKey::MaxRetries, max_retries);
    }
    pub fn get(env: &Env) -> Option<u32> {
        env.storage().instance().get(&StorageKey::MaxRetries)
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
        let key = StorageKey::Tx(id.clone());
        let tx = env.storage().persistent().get(&key).expect("tx not found");
        extend_persistent_ttl(env, &key);
        tx
    }

    pub fn index_anchor_id(env: &Env, anchor_id: &SorobanString, tx_id: &SorobanString) {
        let key = StorageKey::AnchorIdx(anchor_id.clone());
        env.storage().persistent().set(&key, tx_id);
        extend_persistent_ttl(env, &key);
    }

    pub fn find_by_anchor_id(env: &Env, anchor_id: &SorobanString) -> Option<SorobanString> {
        let key = StorageKey::AnchorIdx(anchor_id.clone());
        let val = env.storage().persistent().get(&key);
        if val.is_some() {
            extend_persistent_ttl(env, &key);
        }
        val
    }
}

pub mod settlements {
    use super::*;

    pub fn save(env: &Env, s: &Settlement) {
        let key = StorageKey::Settlement(s.id.clone());
        env.storage().persistent().set(&key, s);
        extend_persistent_ttl(env, &key);
    }

    pub fn get(env: &Env, id: &SorobanString) -> Settlement {
        let key = StorageKey::Settlement(id.clone());
        let s = env
            .storage()
            .persistent()
            .get(&key)
            .expect("settlement not found");
        extend_persistent_ttl(env, &key);
        s
    }
}

pub mod dlq {
    use super::*;

    pub fn push(env: &Env, entry: &DlqEntry) {
        let count_key = StorageKey::DlqCount(0i128);
        let mut count: i128 = env.storage().persistent().get(&count_key).unwrap_or(0i128);
        count += 1;
        env.storage().persistent().set(&count_key, &count);
        extend_persistent_ttl(env, &count_key);
        let key = StorageKey::Dlq(entry.tx_id.clone());
        env.storage().persistent().set(&key, entry);
        extend_persistent_ttl(env, &key);
    }

    pub fn get(env: &Env, tx_id: &SorobanString) -> Option<DlqEntry> {
        let key = StorageKey::Dlq(tx_id.clone());
        let val = env.storage().persistent().get(&key);
        if val.is_some() {
            extend_persistent_ttl(env, &key);
        }
        val
    }

    pub fn remove(env: &Env, tx_id: &SorobanString) {
        let count_key = StorageKey::DlqCount(0i128);
        let mut count: i128 = env.storage().persistent().get(&count_key).unwrap_or(0i128);
        count = count.saturating_sub(1);
        env.storage().persistent().set(&count_key, &count);
        extend_persistent_ttl(env, &count_key);
        env.storage()
            .persistent()
            .remove(&StorageKey::Dlq(tx_id.clone()));
    }

    pub fn get_count(env: &Env) -> i128 {
        let key = StorageKey::DlqCount(0i128);
        let count = env.storage().persistent().get(&key).unwrap_or(0i128);
        extend_persistent_ttl(env, &key);
        count
    }

    #[allow(dead_code)]
    pub fn update(env: &Env, entry: &DlqEntry) {
        let key = StorageKey::Dlq(entry.tx_id.clone());
        env.storage().persistent().set(&key, entry);
        extend_persistent_ttl(env, &key);
    }
}
