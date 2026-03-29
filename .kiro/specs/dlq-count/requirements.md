# Requirements Document

## Introduction

The DLQ (Dead Letter Queue) currently has no O(1) way to report how many entries it holds. Callers must iterate all `Dlq(tx_id)` keys to count them, which is expensive on-chain. This feature adds a persistent `DlqCount` counter that is incremented on every `dlq::push` and decremented on every `dlq::remove`, and exposes the value through a new `get_dlq_count` query on `SynapseContract`.

The `DlqCount(0i128)` storage key and the `dlq::get_count` helper already exist in `src/storage/mod.rs`. The `dlq::push` and `dlq::remove` functions already maintain the counter internally. What is missing is the public contract entry-point that exposes the count to callers.

## Glossary

- **Contract**: `SynapseContract` defined in `src/lib.rs`
- **DLQ**: Dead Letter Queue — the set of `DlqEntry` records stored under `StorageKey::Dlq(tx_id)` keys in persistent storage
- **DlqCount**: The single persistent counter stored under `StorageKey::DlqCount(0i128)` that tracks the total number of live DLQ entries
- **dlq::push**: The storage-layer function in `src/storage/mod.rs` that writes a `DlqEntry` and increments `DlqCount`
- **dlq::remove**: The storage-layer function in `src/storage/mod.rs` that deletes a `DlqEntry` and decrements `DlqCount`
- **dlq::get_count**: The storage-layer helper in `src/storage/mod.rs` that reads and returns the current `DlqCount` value
- **get_dlq_count**: The new public query function to be added to `SynapseContract`

## Requirements

### Requirement 1: Increment DlqCount on push

**User Story:** As a relayer, I want the DLQ counter to be incremented whenever a transaction is pushed to the DLQ, so that the count always reflects the true number of queued entries.

#### Acceptance Criteria

1. WHEN `dlq::push` is called with a `DlqEntry`, THE Contract SHALL increment the `DlqCount` value in persistent storage by exactly 1.
2. WHEN `dlq::push` is called and no `DlqCount` key exists yet, THE Contract SHALL treat the initial count as 0 and store 1 after the push.
3. WHEN `dlq::push` is called N times in sequence, THE Contract SHALL store a `DlqCount` equal to N (assuming no intervening removes).

### Requirement 2: Decrement DlqCount on remove

**User Story:** As a relayer, I want the DLQ counter to be decremented whenever an entry is removed from the DLQ, so that the count stays accurate after retries and completions.

#### Acceptance Criteria

1. WHEN `dlq::remove` is called for an existing DLQ entry, THE Contract SHALL decrement the `DlqCount` value in persistent storage by exactly 1.
2. WHEN `dlq::remove` is called and `DlqCount` is already 0, THE Contract SHALL leave `DlqCount` at 0 (saturating subtraction — no underflow).
3. WHEN `dlq::push` is called N times and `dlq::remove` is called M times (M ≤ N), THE Contract SHALL store a `DlqCount` equal to N − M.

### Requirement 3: Expose DlqCount via query

**User Story:** As an operator or off-chain client, I want to query the total number of DLQ entries without scanning storage, so that I can monitor queue depth cheaply.

#### Acceptance Criteria

1. THE Contract SHALL expose a public function `get_dlq_count(env: Env) -> i128` on `SynapseContract`.
2. WHEN `get_dlq_count` is called and no entries have ever been pushed, THE Contract SHALL return 0.
3. WHEN `get_dlq_count` is called after N pushes and M removes (M ≤ N), THE Contract SHALL return N − M.
4. WHEN `get_dlq_count` is called, THE Contract SHALL extend the TTL of the `DlqCount` persistent storage entry.

### Requirement 4: Counter consistency with DLQ state

**User Story:** As a developer, I want the DlqCount to always equal the number of live `Dlq(tx_id)` entries, so that the counter can be trusted as a source of truth.

#### Acceptance Criteria

1. WHEN a transaction is moved to the DLQ via `mark_failed`, THE Contract SHALL result in `get_dlq_count` returning a value one greater than before the call.
2. WHEN a DLQ entry is removed via `retry_dlq`, THE Contract SHALL result in `get_dlq_count` returning a value one less than before the call.
3. WHEN a DLQ entry is removed via `mark_completed` (which calls `dlq::remove` internally), THE Contract SHALL result in `get_dlq_count` returning a value one less than before the call.

### Requirement 5: Tests

**User Story:** As a developer, I want automated tests for the DlqCount feature, so that regressions are caught early.

#### Acceptance Criteria

1. THE test suite in `tests/contract_test.rs` SHALL include a test that verifies `get_dlq_count` returns 0 on a freshly initialised contract.
2. THE test suite SHALL include a test that verifies `get_dlq_count` increments by 1 after each `mark_failed` call.
3. THE test suite SHALL include a test that verifies `get_dlq_count` decrements by 1 after a successful `retry_dlq` call.
4. THE test suite SHALL include a test that verifies `get_dlq_count` decrements by 1 when `mark_completed` removes a DLQ entry.
5. THE test suite SHALL include a test that verifies `get_dlq_count` never goes below 0.
