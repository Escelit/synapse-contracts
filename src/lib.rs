#![no_std]

mod access;
mod events;
mod storage;
mod types;

use access::{require_admin, require_relayer};
use events::emit;
use soroban_sdk::{contract, contractimpl, Address, Env, String as SorobanString, Vec};
use storage::{assets, deposits, dlq, relayers, settlements};
use types::{DlqEntry, Event, Settlement, Transaction, TransactionStatus};

#[contract]
pub struct SynapseContract;

#[contractimpl]
impl SynapseContract {
    // TODO(#1): prevent re-initialisation — panic if admin already set
    // TODO(#2): emit `Initialized` event on first call
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        storage::admin::set(&env, &admin);
    }

    // TODO(#3): emit `RelayerGranted` event
    // TODO(#4): prevent granting relayer to the zero/invalid address
    pub fn grant_relayer(env: Env, caller: Address, relayer: Address) {
        require_admin(&env, &caller);
        relayers::add(&env, &relayer);
    }

    // TODO(#5): emit `RelayerRevoked` event
    // TODO(#6): panic if revoking a non-existent relayer
    pub fn revoke_relayer(env: Env, caller: Address, relayer: Address) {
        require_admin(&env, &caller);
        relayers::remove(&env, &relayer);
    }

    // TODO(#7): emit `AdminTransferred` event
    // TODO(#8): two-step admin transfer (propose + accept) to prevent lockout
    pub fn transfer_admin(env: Env, caller: Address, new_admin: Address) {
        require_admin(&env, &caller);
        storage::admin::set(&env, &new_admin);
    }

    // TODO(#9): emit `ContractPaused` event
    // TODO(#10): block all state-mutating calls when paused
    pub fn pause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, true);
    }

    // TODO(#11): emit `ContractUnpaused` event
    pub fn unpause(env: Env, caller: Address) {
        require_admin(&env, &caller);
        storage::pause::set(&env, false);
    }

    // TODO(#12): validate asset_code is non-empty and uppercase-alphanumeric only
    // TODO(#13): cap the total number of allowed assets to bound instance storage
    pub fn add_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_admin(&env, &caller);
        assets::add(&env, &asset_code);
        emit(&env, Event::AssetAdded(asset_code));
    }

    // TODO(#14): panic if asset_code is not currently in the allowlist
    pub fn remove_asset(env: Env, caller: Address, asset_code: SorobanString) {
        require_admin(&env, &caller);
        assets::remove(&env, &asset_code);
        emit(&env, Event::AssetRemoved(asset_code));
    }

    // TODO(#15): enforce minimum deposit amount (configurable by admin)
    // TODO(#16): enforce maximum deposit amount (configurable by admin)
    // TODO(#17): validate anchor_transaction_id is non-empty
    // TODO(#18): add `memo` field support (mirrors synapse-core CallbackPayload)
    // TODO(#19): add `memo_type` field support (text | hash | id)
    // TODO(#20): add `callback_type` field (deposit | withdrawal)
    // TODO(#21): bump persistent TTL on AnchorIdx entry after save
    // TODO(#22): bump persistent TTL on Tx entry after save
    pub fn register_deposit(
        env: Env,
        caller: Address,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        amount: i128,
        asset_code: SorobanString,
    ) -> SorobanString {
        require_relayer(&env, &caller);
        assets::require_allowed(&env, &asset_code);

        if let Some(existing) = deposits::find_by_anchor_id(&env, &anchor_transaction_id) {
            return existing;
        }

        let tx = Transaction::new(&env, anchor_transaction_id.clone(), stellar_account, amount, asset_code);
        let id = tx.id.clone();
        deposits::save(&env, &tx);
        deposits::index_anchor_id(&env, &anchor_transaction_id, &id);
        emit(&env, Event::DepositRegistered(id.clone(), anchor_transaction_id));
        id
    }

    // TODO(#23): enforce transition guard — must be Pending
    // TODO(#24): bump Tx TTL on every status update
    pub fn mark_processing(env: Env, caller: Address, tx_id: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Processing;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Processing));
    }

    // TODO(#25): enforce transition guard — must be Processing
    pub fn mark_completed(env: Env, caller: Address, tx_id: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Completed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Completed));
    }

    // TODO(#26): enforce transition guard — must be Pending or Processing
    // TODO(#27): cap max retry_count; emit `MaxRetriesExceeded` when hit
    // TODO(#28): validate error_reason is non-empty
    pub fn mark_failed(env: Env, caller: Address, tx_id: SorobanString, error_reason: SorobanString) {
        require_relayer(&env, &caller);
        let mut tx = deposits::get(&env, &tx_id);
        tx.status = TransactionStatus::Failed;
        tx.updated_ledger = env.ledger().sequence();
        deposits::save(&env, &tx);
        let entry = DlqEntry::new(&env, tx_id.clone(), error_reason.clone());
        dlq::push(&env, &entry);
        emit(&env, Event::MovedToDlq(tx_id, error_reason));
    }

    // TODO(#29): implement — reset tx status to Pending, increment retry_count
    // TODO(#30): remove DLQ entry after successful retry
    // TODO(#31): emit `DlqRetried` event
    // TODO(#32): only admin OR original relayer should be able to retry
    pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
        require_admin(&env, &caller);
        let _ = (env, tx_id);
        panic!("not implemented")
    }

    // TODO(#33): verify each tx_id exists and has status Completed
    // TODO(#34): verify no tx_id is already linked to a settlement
    // TODO(#35): write settlement_id back onto each Transaction
    // TODO(#36): verify total_amount matches sum of tx amounts on-chain
    // TODO(#37): verify period_start <= period_end
    // TODO(#38): bump Settlement TTL after save
    // TODO(#39): emit per-tx `Settled` event in addition to batch event
    pub fn finalize_settlement(
        env: Env,
        caller: Address,
        asset_code: SorobanString,
        tx_ids: Vec<SorobanString>,
        total_amount: i128,
        period_start: u64,
        period_end: u64,
    ) -> SorobanString {
        require_relayer(&env, &caller);
        let s = Settlement::new(&env, asset_code.clone(), tx_ids, total_amount, period_start, period_end);
        let id = s.id.clone();
        settlements::save(&env, &s);
        emit(&env, Event::SettlementFinalized(id.clone(), asset_code, total_amount));
        id
    }

    // TODO(#40): add `get_dlq_entry(tx_id)` query
    // TODO(#41): add `get_admin()` query
    // TODO(#42): add `is_paused()` query
    // TODO(#43): add `get_min_deposit()` query
    // TODO(#44): add `get_max_deposit()` query

    pub fn get_transaction(env: Env, tx_id: SorobanString) -> Transaction {
        deposits::get(&env, &tx_id)
    }

    pub fn get_settlement(env: Env, settlement_id: SorobanString) -> Settlement {
        settlements::get(&env, &settlement_id)
    }

    pub fn is_asset_allowed(env: Env, asset_code: SorobanString) -> bool {
        assets::is_allowed(&env, &asset_code)
    }

    pub fn is_relayer(env: Env, address: Address) -> bool {
        relayers::has(&env, &address)
    }
}
