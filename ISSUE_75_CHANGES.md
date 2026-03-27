# Issue #75: Add test confirming settlement panics if a tx is already settled

## Summary
This commit adds a regression test for issue #34 that confirms `finalize_settlement` panics when trying to settle a transaction that has already been settled.

## Changes Made

### Added regression test (src/lib.rs)

**Location**: At the end of the tests module, after the existing `test_finalize_settlement_panics_when_transaction_already_settled` test

**Change**: Add this test function:

```rust
#[test]
#[should_panic(expected = "transaction already settled")]
fn test_finalize_settlement_panics_when_double_settle() {
    let env = Env::default();
    let (admin, contract_id) = setup(&env);
    let client = SynapseContractClient::new(&env, &contract_id);
    let relayer = Address::generate(&env);
    let stellar = Address::generate(&env);
    let asset = SorobanString::from_str(&env, "USD");
    let anchor_id = SorobanString::from_str(&env, "double-settle");

    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &asset);
    let tx_id = client.register_deposit(
        &relayer,
        &anchor_id,
        &stellar,
        &100i128,
        &asset,
        &None,
    );

    // First settlement - should succeed
    client.finalize_settlement(
        &relayer,
        &asset,
        &vec![&env, tx_id.clone()],
        &100i128,
        &1u64,
        &2u64,
    );

    // Second settlement - should panic
    client.finalize_settlement(
        &relayer,
        &asset,
        &vec![&env, tx_id],
        &100i128,
        &3u64,
        &4u64,
    );
}
```

## What this accomplishes

1. **Regression Test**: The test creates a transaction, settles it successfully with the first call to `finalize_settlement`, then attempts to settle the same transaction again with a second call.

2. **Validates Existing Logic**: This test confirms that the existing validation logic in `finalize_settlement` (which checks `if tx.settlement_id.len() > 0 { panic!("transaction already settled"); }`) works correctly.

3. **Addresses Issue #34**: This provides the regression test requested to ensure transactions cannot be double-settled.

4. **Different from Existing Test**: Unlike the existing `test_finalize_settlement_panics_when_transaction_already_settled` test which manually manipulates storage to set a settlement_id, this test actually calls the settlement function twice to simulate the real-world scenario.

## Test Flow

1. **Setup**: Create admin, relayer, asset, and register a deposit transaction
2. **First Settlement**: Call `finalize_settlement` - should succeed and set the settlement_id on the transaction
3. **Second Settlement**: Call `finalize_settlement` again with the same transaction - should panic with "transaction already settled"

## Files Modified
- `src/lib.rs` - Added regression test

## Lines Changed
- Approximately 35 lines added for the test
- Total: ~35 lines of focused changes

This is a minimal, focused commit that only addresses the specific requirement for a double settlement regression test.