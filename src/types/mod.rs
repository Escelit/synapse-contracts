use alloc::format;
use soroban_sdk::{contracttype, Address, Env, String as SorobanString, Vec};
extern crate alloc;
use alloc::format;

// TODO(#45): replace generate_id with hash(anchor_transaction_id) for determinism

pub const MAX_RETRIES: u32 = 5;
// TODO(#46): add `Cancelled` status for user-initiated cancellations
// TODO(#47): add `memo: Option<SorobanString>` field to Transaction
// TODO(#48): add `memo_type: Option<SorobanString>` field to Transaction
// TODO(#49): add `callback_type: Option<SorobanString>` field to Transaction
// TODO(#50): store `relayer: Address` on Transaction (who registered it)

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[contracttype]
#[derive(Clone)]
pub struct Transaction {
    pub id: SorobanString,
    pub anchor_transaction_id: SorobanString,
    pub stellar_account: Address,
    pub relayer: Address, // #50: who registered this deposit
    pub amount: i128,
    pub asset_code: SorobanString,
    pub status: TransactionStatus,
    pub created_ledger: u32,
    pub updated_ledger: u32,
    pub settlement_id: SorobanString, // empty = unsettled
    pub memo: Option<SorobanString>,
    pub memo_type: Option<SorobanString>,
    pub callback_type: Option<SorobanString>,
}

impl Transaction {
    pub fn new(
        env: &Env,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        relayer: Address,
        amount: i128,
        asset_code: SorobanString,
        memo: Option<SorobanString>,
    ) -> Self {
        let ledger = env.ledger().sequence();
        Self {
            id: generate_id(env, &anchor_transaction_id),
            anchor_transaction_id,
            stellar_account,
            relayer,
            amount,
            asset_code,
            status: TransactionStatus::Pending,
            created_ledger: ledger,
            updated_ledger: ledger,
            settlement_id: SorobanString::from_str(env, ""),
            memo,
            memo_type: None,
            callback_type: None,
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub struct Settlement {
    pub id: SorobanString,
    pub asset_code: SorobanString,
    pub tx_ids: Vec<SorobanString>,
    pub total_amount: i128,
    pub period_start: u64,
    pub period_end: u64,
    pub created_ledger: u32,
}

impl Settlement {
    pub fn new(
        env: &Env,
        asset_code: SorobanString,
        tx_ids: Vec<SorobanString>,
        total_amount: i128,
        period_start: u64,
        period_end: u64,
    ) -> Self {
        Self {
            id: generate_settlement_id(env),
            asset_code,
            tx_ids,
            total_amount,
            period_start,
            period_end,
            created_ledger: env.ledger().sequence(),
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub struct DlqEntry {
    pub tx_id: SorobanString,
    pub error_reason: SorobanString,
    pub retry_count: u32,
    pub moved_at_ledger: u32,
    pub last_retry_ledger: u32,
}

impl DlqEntry {
    pub fn new(env: &Env, tx_id: SorobanString, error_reason: SorobanString) -> Self {
        Self {
            tx_id,
            error_reason,
            retry_count: 0,
            moved_at_ledger: env.ledger().sequence(),
            last_retry_ledger: 0,
        }
    }
}

/// Contract events — one variant per state change.
// TODO(#51): add `RelayerGranted(Address)` variant
// TODO(#53): add `Initialized(Address)` variant
// TODO(#54): add `ContractPaused` / `ContractUnpaused` variants
// TODO(#56): add `MaxRetriesExceeded(SorobanString)` variant
// TODO(#57): add `AdminTransferred(Address, Address)` variant
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Initialized(Address),                                    // (admin)
    DepositRegistered(SorobanString, SorobanString),         // (tx_id, anchor_id)
    StatusUpdated(SorobanString, TransactionStatus),         // (tx_id, new_status)
    MovedToDlq(SorobanString, SorobanString),                // (tx_id, error_reason)
    DlqRetried(SorobanString),                               // (tx_id)
    SettlementFinalized(SorobanString, SorobanString, i128), // (settlement_id, asset_code, total)
    AssetAdded(SorobanString),
    AssetRemoved(SorobanString),
    RelayerRevoked(Address),
}

fn generate_id(env: &Env) -> SorobanString {
    let ts = env.ledger().timestamp();
    let seq = env.ledger().sequence();
    let mut data = [0u8; 12];
    data[..8].copy_from_slice(&ts.to_be_bytes());
    data[8..12].copy_from_slice(&seq.to_be_bytes());
    let hash = env.crypto().sha256(&soroban_sdk::Bytes::from_slice(env, &data));
    let bytes = hash.to_array();
    // encode first 16 bytes as 32-char hex
    let mut hex = [0u8; 32];
    const HEX: &[u8] = b"0123456789abcdef";
    for i in 0..16 {
        hex[i * 2]     = HEX[(bytes[i] >> 4) as usize];
        hex[i * 2 + 1] = HEX[(bytes[i] & 0xf) as usize];
    }
    SorobanString::from_bytes(env, &hex)
}
