use soroban_sdk::{contracttype, Address, Env, String as SorobanString, Vec};

// TODO(#45): replace generate_id with hash(anchor_transaction_id) for determinism

pub const MAX_RETRIES: u32 = 5;
// TODO(#46): add `Cancelled` status for user-initiated cancellations

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct Transaction {
    pub id: SorobanString,
    pub anchor_transaction_id: SorobanString,
    pub stellar_account: Address,
    pub relayer: Address,
    pub amount: i128,
    pub asset_code: SorobanString,
    pub memo: Option<SorobanString>,
    pub status: TransactionStatus,
    pub created_ledger: u32,
    pub updated_ledger: u32,
    pub settlement_id: SorobanString,
    pub memo: Option<SorobanString>,
    pub memo_type: Option<SorobanString>,
    pub callback_type: Option<SorobanString>,
}

impl Transaction {
    pub fn new(
        env: &Env,
        id: SorobanString,
        anchor_transaction_id: SorobanString,
        stellar_account: Address,
        relayer: Address,
        amount: i128,
        asset_code: SorobanString,
        callback_type: Option<SorobanString>,
        memo: Option<SorobanString>,
        memo_type: Option<SorobanString>,
    ) -> Self {
        let ledger = env.ledger().sequence();
        Self {
            id,
            anchor_transaction_id,
            stellar_account,
            relayer,
            amount,
            asset_code,
            memo,
            status: TransactionStatus::Pending,
            created_ledger: ledger,
            updated_ledger: ledger,
            settlement_id: SorobanString::from_str(env, ""),
            callback_type,
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
        id: SorobanString,
        asset_code: SorobanString,
        tx_ids: Vec<SorobanString>,
        total_amount: i128,
        period_start: u64,
        period_end: u64,
    ) -> Self {
        Self {
            id,
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

/// Contract events - one variant per state change.
// TODO(#54): add `ContractPaused` / `ContractUnpaused` variants
// TODO(#55): add `DlqRetried(SorobanString)` variant
// TODO(#56): add `MaxRetriesExceeded(SorobanString)` variant
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    // Lifecycle
    Initialized(Address),                                    // (admin)

    // Relayer management
    RelayerGranted(Address),                                 // (relayer)
    DepositRegistered(SorobanString, SorobanString),         // (tx_id, anchor_id)
    StatusUpdated(SorobanString, TransactionStatus),         // (tx_id, new_status)
    SettlementFinalized(SorobanString, SorobanString, i128), // (settlement_id, asset_code, total)

    // Pause
    ContractPaused(Address),                                 // (admin)
    ContractUnpaused(Address),                               // (admin)

    // DLQ
    MovedToDlq(SorobanString, SorobanString),                // (tx_id, error_reason)
    DlqRetried(SorobanString),                               // (tx_id)
    SettlementFinalized(SorobanString, SorobanString, i128), // (settlement_id, asset_code, total)
    Settled(SorobanString, SorobanString),                   // (tx_id, settlement_id)
    AssetAdded(SorobanString),
    AssetRemoved(SorobanString),
}

fn generate_id(env: &Env, _anchor_transaction_id: &SorobanString) -> SorobanString {
    let ts = env.ledger().timestamp();
    let seq = env.ledger().sequence();
    let mut data = [0u8; 12];
    data[..8].copy_from_slice(&ts.to_be_bytes());
    data[8..12].copy_from_slice(&seq.to_be_bytes());
    let hash = env
        .crypto()
        .sha256(&soroban_sdk::Bytes::from_slice(env, &data));
    let bytes = hash.to_array();
    let mut hex = [0u8; 32];
    const HEX: &[u8] = b"0123456789abcdef";
    for i in 0..16 {
        hex[i * 2] = HEX[(bytes[i] >> 4) as usize];
        hex[i * 2 + 1] = HEX[(bytes[i] & 0x0f) as usize];
    }
    SorobanString::from_bytes(env, &hex)
}

fn generate_settlement_id(env: &Env) -> SorobanString {
    SorobanString::from_str(
        env,
        &format!(
            "settlement-{}-{}",
            env.ledger().timestamp(),
            env.ledger().sequence()
        ),
    )
}
