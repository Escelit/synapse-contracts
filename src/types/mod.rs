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
#[derive(Clone, PartialEq)]
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
    pub amount: i128,
    pub asset_code: SorobanString,
    pub status: TransactionStatus,
    pub created_ledger: u32,
    pub updated_ledger: u32,
    pub settlement_id: SorobanString, // empty = unsettled
    pub callback_type: Option<SorobanString>,
}

impl Transaction {
    pub fn new(
        env: &Env,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        amount: i128,
        asset_code: SorobanString,
    ) -> Self {
        let ledger = env.ledger().sequence();
        Self {
            id: generate_id(env),
            anchor_transaction_id,
            stellar_account,
            amount,
            asset_code,
            status: TransactionStatus::Pending,
            created_ledger: ledger,
            updated_ledger: ledger,
            settlement_id: SorobanString::from_str(env, ""),
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
            id: generate_id(env),
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
// TODO(#52): add `RelayerRevoked(Address)` variant
// TODO(#54): add `ContractPaused` / `ContractUnpaused` variants
// TODO(#55): add `DlqRetried(SorobanString)` variant
// TODO(#56): add `MaxRetriesExceeded(SorobanString)` variant
// TODO(#57): add `AdminTransferred(Address, Address)` variant
#[contracttype]
#[derive(Clone)]
pub enum Event {
    Initialized(Address),                                    // (admin)
    DepositRegistered(SorobanString, SorobanString), // (tx_id, anchor_id)
    StatusUpdated(SorobanString, TransactionStatus),  // (tx_id, new_status)
    MovedToDlq(SorobanString, SorobanString),         // (tx_id, error_reason)
    SettlementFinalized(SorobanString, SorobanString, i128), // (settlement_id, asset_code, total)
    AssetAdded(SorobanString),
    AssetRemoved(SorobanString),
    DlqRetried(SorobanString),
    MaxRetriesExceeded(SorobanString),
}

fn generate_id(env: &Env) -> SorobanString {
    let ts = env.ledger().timestamp();
    let seq = env.ledger().sequence() as u64;
    let combined = ts * 1_000_000 + seq;
    // Format combined u64 as decimal string without std::format!
    let mut buf = [0u8; 20]; // max u64 is 20 digits
    let mut n = combined;
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while n > 0 {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
    }
    let s = core::str::from_utf8(&buf[i..]).unwrap_or("0");
    SorobanString::from_str(env, s)
}
