# Ensure dlq::remove is called after successful retry in retry_dlq

## Summary
This commit ensures that DLQ entries are properly removed after a successful retry to prevent duplicate processing and maintain clean DLQ state.

## Problem
The current `retry_dlq` implementation updates the DLQ entry with incremented retry count but doesn't remove it from the DLQ. This causes:
- Old DLQ entries to remain in storage
- Potential for duplicate retry processing
- DLQ storage bloat over time

## Changes Made

### 1. Update `retry_dlq` function (src/lib.rs)

**Current implementation**:
```rust
pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
    require_admin(&env, &caller);
    
    let mut entry = dlq::get(&env, &tx_id).expect("dlq entry not found");
    let mut tx = deposits::get(&env, &tx_id);
    
    tx.status = TransactionStatus::Pending;
    tx.updated_ledger = env.ledger().sequence();
    
    entry.retry_count += 1;
    entry.last_retry_ledger = env.ledger().sequence();
    
    deposits::save(&env, &tx);
    dlq::push(&env, &entry);  // This updates the entry but doesn't remove it
    
    emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Pending));
}
```

**Updated implementation**:
```rust
pub fn retry_dlq(env: Env, caller: Address, tx_id: SorobanString) {
    require_admin(&env, &caller);
    
    let entry = dlq::get(&env, &tx_id).expect("dlq entry not found");
    let mut tx = deposits::get(&env, &tx_id);
    
    tx.status = TransactionStatus::Pending;
    tx.updated_ledger = env.ledger().sequence();
    
    deposits::save(&env, &tx);
    dlq::remove(&env, &tx_id);  // Remove the DLQ entry after successful retry
    
    emit(&env, Event::StatusUpdated(tx_id, TransactionStatus::Pending));
}
```

### 2. Add tests to confirm DLQ removal

**Test 1**: Verify DLQ entry is removed after successful retry:

```rust
#[test]
fn test_retry_dlq_removes_dlq_entry() {
    let env = Env::default();
    let (client, relayer, tx_id) = setup_relayer_deposit(&env, "dlq-remove-test");
    let admin = env.as_contract(&client.address, || storage::admin::get(&env));
    let err = SorobanString::from_str(&env, "test-error");
    
    // 1. Mark transaction as failed (creates DLQ entry)
    client.mark_failed(&relayer, &tx_id, &err);
    
    // 2. Verify DLQ entry exists
    let dlq_entry_before = env.as_contract(&client.address, || {
        storage::dlq::get(&env, &tx_id)
    });
    assert!(dlq_entry_before.is_some());
    
    // 3. Retry DLQ
    client.retry_dlq(&admin, &tx_id);
    
    // 4. Verify DLQ entry is removed
    let dlq_entry_after = env.as_contract(&client.address, || {
        storage::dlq::get(&env, &tx_id)
    });
    assert!(dlq_entry_after.is_none());
    
    // 5. Verify transaction status is reset to Pending
    let tx = client.get_transaction(&tx_id);
    assert!(matches!(tx.status, TransactionStatus::Pending));
}
```

**Test 2**: Verify retry cannot be called twice on same transaction:

```rust
#[test]
#[should_panic(expected = "dlq entry not found")]
fn test_retry_dlq_panics_when_no_dlq_entry() {
    let env = Env::default();
    let (client, relayer, tx_id) = setup_relayer_deposit(&env, "no-dlq-test");
    let admin = env.as_contract(&client.address, || storage::admin::get(&env));
    let err = SorobanString::from_str(&env, "test-error");
    
    // 1. Mark transaction as failed
    client.mark_failed(&relayer, &tx_id, &err);
    
    // 2. First retry - should succeed
    client.retry_dlq(&admin, &tx_id);
    
    // 3. Second retry - should panic because DLQ entry was removed
    client.retry_dlq(&admin, &tx_id);
}
```

**Test 3**: Verify existing retry_dlq_success test still works:

```rust
#[test]
fn test_retry_dlq_success_updated() {
    let env = Env::default();
    let (client, relayer, tx_id) = setup_relayer_deposit(&env, "retry-success");
    let admin = env.as_contract(&client.address, || storage::admin::get(&env));
    let err = SorobanString::from_str(&env, "failed-initially");
    
    // 1. Mark as failed
    client.mark_failed(&relayer, &tx_id, &err);
    let tx_failed = client.get_transaction(&tx_id);
    assert!(matches!(tx_failed.status, TransactionStatus::Failed));
    
    // 2. Retry DLQ
    env.ledger().set_sequence_number(100);
    client.retry_dlq(&admin, &tx_id);
    
    // 3. Verify Transaction status is reset
    let tx_retried = client.get_transaction(&tx_id);
    assert!(matches!(tx_retried.status, TransactionStatus::Pending));
    assert_eq!(tx_retried.updated_ledger, 100);
    
    // 4. Verify DLQ Entry is removed (not just updated)
    let entry = env.as_contract(&client.address, || {
        storage::dlq::get(&env, &tx_id)
    });
    assert!(entry.is_none());
}
```

## What this accomplishes

1. **Prevents Duplicate Processing**: DLQ entries are removed after successful retry, preventing the same transaction from being retried multiple times
2. **Clean Storage**: Removes processed DLQ entries to prevent storage bloat
3. **Proper State Management**: Ensures DLQ only contains transactions that actually need retry
4. **Consistent Behavior**: Aligns with the expected behavior that a successful retry removes the need for further DLQ processing

## Files Modified
- `src/lib.rs` - Updated `retry_dlq` function and added tests

## Lines Changed
- Remove `entry.retry_count += 1` and `entry.last_retry_ledger` updates (no longer needed)
- Remove `dlq::push(&env, &entry)` call
- Add `dlq::remove(&env, &tx_id)` call
- Add approximately 60 lines of tests
- Total: ~65 lines changed

This implementation ensures proper DLQ cleanup and prevents duplicate retry processing while maintaining all existing functionality.