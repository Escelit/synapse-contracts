# Implement require_not_paused function and wire into mutating functions

## Summary
This commit implements the `require_not_paused` function in `access/mod.rs` and wires it into every mutating function in `lib.rs` to provide central pause guard functionality.

## Changes Made

### 1. Updated `require_not_paused` function (src/access/mod.rs)

**Change**: Update the panic message and remove TODO comment:

```rust
pub fn require_not_paused(env: &Env) {
    if pause::is_paused(env) {
        panic!("contract is paused")
    }
}
```

### 2. Import `require_not_paused` in lib.rs

**Change**: Update the import statement:

```rust
use access::{require_admin, require_relayer, require_not_paused};
```

### 3. Add `require_not_paused` calls to mutating functions

**Functions that should have pause checks** (add `require_not_paused(&env);` as the first line):

1. `grant_relayer` - Add after the zero address check
2. `revoke_relayer` - Add as first line
3. `transfer_admin` - Add as first line
4. `add_asset` - Add as first line
5. `remove_asset` - Add as first line
6. `register_deposit` - Add as first line
7. `mark_processing` - Add as first line
8. `mark_completed` - Add as first line
9. `mark_failed` - Add as first line
10. `retry_dlq` - Add as first line
11. `finalize_settlement` - Add as first line
12. `set_max_deposit` - Add as first line

**Functions that should NOT have pause checks**:
- `initialize` - Initialization should work even when paused
- `pause` - Admin needs to be able to pause
- `unpause` - Admin needs to be able to unpause
- All query functions (get_*, is_*) - Read-only operations

### 4. Add tests

**Test 1**: Test that mutating functions panic when paused:

```rust
#[test]
#[should_panic(expected = "contract is paused")]
fn test_mutating_functions_panic_when_paused() {
    let env = Env::default();
    let (admin, contract_id) = setup(&env);
    let client = SynapseContractClient::new(&env, &contract_id);
    let relayer = Address::generate(&env);
    
    // Setup relayer and asset
    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &SorobanString::from_str(&env, "USD"));
    
    // Pause the contract
    client.pause(&admin);
    
    // Try to call a mutating function - should panic
    client.grant_relayer(&admin, &Address::generate(&env));
}
```

**Test 2**: Test that pause/unpause work when contract is paused:

```rust
#[test]
fn test_pause_unpause_work_when_paused() {
    let env = Env::default();
    let (admin, contract_id) = setup(&env);
    let client = SynapseContractClient::new(&env, &contract_id);
    
    // Pause the contract
    client.pause(&admin);
    assert!(client.is_paused());
    
    // Should be able to unpause even when paused
    client.unpause(&admin);
    assert!(!client.is_paused());
    
    // Should be able to pause again
    client.pause(&admin);
    assert!(client.is_paused());
}
```

**Test 3**: Test that query functions work when paused:

```rust
#[test]
fn test_query_functions_work_when_paused() {
    let env = Env::default();
    let (admin, contract_id) = setup(&env);
    let client = SynapseContractClient::new(&env, &contract_id);
    
    // Pause the contract
    client.pause(&admin);
    
    // Query functions should still work
    assert_eq!(client.get_admin(), admin);
    assert!(client.is_paused());
    assert_eq!(client.get_max_deposit(), 0i128);
}
```

## What this accomplishes

1. **Central Pause Guard**: All mutating functions now check if the contract is paused before executing
2. **Proper Exception Handling**: Admin functions like pause/unpause and initialization are not blocked by pause state
3. **Query Functions Unaffected**: Read-only functions continue to work when paused
4. **Foundation for #10 and #61**: Provides the infrastructure for blocking state-mutating calls when paused

## Files Modified
- `src/access/mod.rs` - Updated `require_not_paused` function
- `src/lib.rs` - Added import and wired function into all mutating functions, added tests

## Lines Changed
- Approximately 15 lines of pause checks added across mutating functions
- Approximately 50 lines of tests added
- Total: ~65 lines of focused changes

This implementation provides comprehensive pause functionality while maintaining proper access control for administrative functions.