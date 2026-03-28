use crate::types::{DlqEntry, Settlement, Transaction};
use soroban_sdk::{contracttype, Address, Env, String as SorobanString};

pub const TX_TTL_THRESHOLD: u32 = 17_280;
pub const TX_TTL_EXTEND_TO: u32 = 172_800;
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
    TempLock(SorobanString),
}

fn extend_instance_ttl(env: &Env) {
    env.storage().instance().extend_ttl(TX_TTL_THRESHOLD, TX_TTL_EXTEND_TO);
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

    pub fn add(env: &Env, relayer: &Address) {
        env.storage()
            .instance()
            .set(&StorageKey::Relayer(relayer.clone()), &true);
    }

    pub fn remove(env: &Env, relayer: &Address) {
        env.storage()
            .instance()
            .remove(&StorageKey::Relayer(relayer.clone()));
    }

    pub fn has(env: &Env, relayer: &Address) -> bool {
        env.storage()
            .instance()
            .has(&StorageKey::Relayer(relayer.clone()))
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

    fn set_count(env: &Env, count: u32) {
        env.storage().instance().set(&StorageKey::AssetCount, &count);
    }

    pub fn add(env: &Env, code: &SorobanString) {
        if is_allowed(env, code) {
            return;
        }

        let count = count(env);
        if count >= MAX_ASSETS {
            panic!("asset cap reached");
        }

        env.storage()
            .instance()
            .set(&StorageKey::Asset(code.clone()), &true);
        set_count(env, count + 1);
    }

    pub fn remove(env: &Env, code: &SorobanString) {
        if !is_allowed(env, code) {
            return;
        }

        env.storage()
            .instance()
            .remove(&StorageKey::Asset(code.clone()));
        set_count(env, count(env).saturating_sub(1));
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

pub mod deposits {
    use super::*;

    pub fn save(env: &Env, tx: &Transaction) {
        let key = StorageKey::Tx(tx.id.clone());
        env.storage().persistent().set(&key, tx);
        extend_persistent_ttl(env, &key);

        let anchor_key = StorageKey::AnchorIdx(tx.anchor_transaction_id.clone());
        if env.storage().persistent().has(&anchor_key) {
            extend_persistent_ttl(env, &anchor_key);
        }
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
        let value = env.storage().persistent().get(&key);
        if value.is_some() {
            extend_persistent_ttl(env, &key);
        }
        value
    }
}

pub mod settlements {
    use super::*;

    const SETTLEMENT_TTL_THRESHOLD: u32 = 535_679;
    const SETTLEMENT_TTL_EXTEND_TO: u32 = 535_679;

    pub fn save(env: &Env, settlement: &Settlement) {
        let key = StorageKey::Settlement(settlement.id.clone());
        env.storage().persistent().set(&key, settlement);
        env.storage()
            .persistent()
            .extend_ttl(&key, SETTLEMENT_TTL_THRESHOLD, SETTLEMENT_TTL_EXTEND_TO);
    }

    pub fn get(env: &Env, id: &SorobanString) -> Settlement {
        let key = StorageKey::Settlement(id.clone());
        let settlement = env
            .storage()
            .persistent()
            .get(&key)
            .expect("settlement not found");
        env.storage()
            .persistent()
            .extend_ttl(&key, SETTLEMENT_TTL_THRESHOLD, SETTLEMENT_TTL_EXTEND_TO);
        settlement
    }
}

pub mod dlq {
    use super::*;

    pub fn push(env: &Env, entry: &DlqEntry) {
        let key = StorageKey::Dlq(entry.tx_id.clone());
        let count_key = StorageKey::DlqCount(0i128);
        if !env.storage().persistent().has(&key) {
            let count: i128 = env.storage().persistent().get(&count_key).unwrap_or(0i128) + 1;
            env.storage().persistent().set(&count_key, &count);
            extend_persistent_ttl(env, &count_key);
        }

        env.storage().persistent().set(&key, entry);
        extend_persistent_ttl(env, &key);
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
        let key = StorageKey::Dlq(tx_id.clone());
        if !env.storage().persistent().has(&key) {
            return;
        }

        let count_key = StorageKey::DlqCount(0i128);
        let count: i128 = env.storage().persistent().get(&count_key).unwrap_or(0i128);
        env.storage()
            .persistent()
            .set(&count_key, &count.saturating_sub(1));
        extend_persistent_ttl(env, &count_key);
        env.storage().persistent().remove(&key);
    }

    pub fn get_count(env: &Env) -> i128 {
        let key = StorageKey::DlqCount(0i128);
        let count = env.storage().persistent().get(&key).unwrap_or(0i128);
        if env.storage().persistent().has(&key) {
            extend_persistent_ttl(env, &key);
        }
        count
    }

    pub fn update(env: &Env, entry: &DlqEntry) {
        let key = StorageKey::Dlq(entry.tx_id.clone());
        env.storage().persistent().set(&key, entry);
        extend_persistent_ttl(env, &key);
    }
}

pub mod temp_lock {
    use super::*;

    const TEMP_LOCK_THRESHOLD: u32 = 3_600;
    const TEMP_LOCK_EXTEND_TO: u32 = 7_200;

    pub fn lock(env: &Env, key: &SorobanString) {
        let lock_key = StorageKey::TempLock(key.clone());
        if env.storage().temporary().has(&lock_key) {
            panic!("idempotency lock active");
        }
        env.storage().temporary().set(&lock_key, &true);
        env.storage()
            .temporary()
            .extend_ttl(&lock_key, TEMP_LOCK_THRESHOLD, TEMP_LOCK_EXTEND_TO);
    }

    pub fn unlock(env: &Env, key: &SorobanString) {
        env.storage()
            .temporary()
            .remove(&StorageKey::TempLock(key.clone()));
    }

    pub fn is_locked(env: &Env, key: &SorobanString) -> bool {
        env.storage()
            .temporary()
            .has(&StorageKey::TempLock(key.clone()))
    }
}

pub use temp_lock::{is_locked as is_temp_locked, lock as lock_temp, unlock as unlock_temp};
