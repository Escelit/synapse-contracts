# Design Document: dlq-count

## Overview

Add a `get_dlq_count` public query to `SynapseContract` that returns the current value of the `DlqCount` persistent counter. The counter is already maintained by `dlq::push` and `dlq::remove` in `src/storage/mod.rs`. The only missing piece is wiring the existing `dlq::get_count` storage helper through to a public contract entry-point.

## Architecture

```
Caller (off-chain / other contract)
        │
        │  get_dlq_count()
        ▼
SynapseContract  (src/lib.rs)
        │
        │  dlq::get_count(&env)
        ▼
storage::dlq  (src/storage/mod.rs)
        │
        │  env.storage().persistent().get(DlqCount(0i128))
        ▼
Soroban Persistent Storage
```

The counter is updated on every `dlq::push` (+1) and `dlq::remove` (saturating −1). Both paths already exist; no changes to storage logic are needed.

## Components and Interfaces

### `SynapseContract::get_dlq_count` (new)

Location: `src/lib.rs`, inside `#[contractimpl] impl SynapseContract`

```rust
pub fn get_dlq_count(env: Env) -> i128 {
    dlq::get_count(&env)
}
```

- No auth required — read-only query.
- No pause check — counter reads are always safe.
- Delegates entirely to the existing `dlq::get_count` helper.

### `storage::dlq::get_count` (existing — no changes)

```rust
pub fn get_count(env: &Env) -> i128 {
    let count_key = StorageKey::DlqCount(0i128);
    let count = env.storage().persistent().get(&count_key).unwrap_or(0i128);
    extend_persistent_ttl(env, &count_key);
    count
}
```

Already extends TTL on read, satisfying Requirement 3.4.

### `storage::dlq::push` / `dlq::remove` (existing — no changes)

Both already read-modify-write `DlqCount(0i128)` with `saturating_sub` on remove.

## Data Models

No new data types. The counter uses the existing `StorageKey::DlqCount(i128)` variant with the fixed key `DlqCount(0i128)` stored in persistent storage as an `i128`.

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

Property 1: Push increments count by 1
*For any* contract state and any valid `DlqEntry`, calling `dlq::push` once should increase the value returned by `get_dlq_count` by exactly 1.
**Validates: Requirements 1.1, 1.3**

Property 2: Remove decrements count by 1
*For any* contract state where `DlqCount > 0`, calling `dlq::remove` once should decrease the value returned by `get_dlq_count` by exactly 1.
**Validates: Requirements 2.1, 2.3**

Property 3: Count never goes below zero
*For any* sequence of `dlq::push` and `dlq::remove` calls, the value returned by `get_dlq_count` SHALL never be negative.
**Validates: Requirements 2.2**

Property 4: Push then remove is identity
*For any* contract state, pushing N entries then removing all N entries should leave `get_dlq_count` equal to its value before the pushes (round-trip property).
**Validates: Requirements 1.1, 2.1, 4.1, 4.2**

Property 5: Count equals live DLQ entry count
*For any* sequence of `mark_failed` and `retry_dlq` / `mark_completed` calls, `get_dlq_count` should equal the number of `tx_id`s for which a DLQ entry currently exists.
**Validates: Requirements 4.1, 4.2, 4.3**

## Error Handling

- `get_dlq_count` has no failure modes; it returns 0 when the key is absent.
- Underflow is prevented by `saturating_sub` in `dlq::remove` (Requirement 2.2).
- No new error paths are introduced.

## Testing Strategy

### Unit / Integration Tests (`tests/contract_test.rs`)

Specific examples and edge cases:

- `get_dlq_count_returns_zero_on_fresh_contract` — baseline, no entries pushed.
- `get_dlq_count_increments_on_mark_failed` — push one entry, assert count == 1.
- `get_dlq_count_decrements_on_retry_dlq` — push then retry, assert count == 0.
- `get_dlq_count_decrements_on_mark_completed` — push then complete, assert count == 0.
- `get_dlq_count_never_goes_below_zero` — remove on empty DLQ, assert count == 0.

### Property-Based Tests

The Soroban test environment is deterministic and does not support an external PBT library (e.g., `proptest`) without `std`. Properties are therefore validated through parameterised example tests that cover the key invariants:

- **Property 1 & 2**: Loop N times calling `mark_failed`, assert count == N; loop M times calling `retry_dlq`, assert count == N − M.
- **Property 3**: Attempt `dlq::remove` when count is 0, assert count remains 0.
- **Property 4**: Push N, remove N, assert count equals pre-push value.
- **Property 5**: After any mix of push/remove, assert `get_dlq_count` == number of entries that still have a DLQ record.

Each test MUST include a comment referencing the property it validates:
`// Feature: dlq-count, Property <N>: <property_text>`
