# Issue #76: Add test for total amount mismatch in finalize_settlement

## Summary
This commit adds a regression test for issue #36 that confirms `finalize_settlement` panics when `total_amount` doesn't match the sum of transaction amounts.

## Changes Made

### 1. Added validation logic in `finalize_settlement` function (src/lib.rs)

**Location**: Around line 200 in the `finalize_settlement` function

**Change**: Replace the existing loop that checks for already settled transactions with:

```rust
let n = tx_ids.len();
let mut i: u32 = 0;
let mut sum: i128 = 0;
while i < n {
    let tx_id = tx_ids.get(i).unwrap();
    let tx = deposits::get(&env, &tx_id);
    if tx.settlement_id.len() > 0 {
        panic!("transaction already settled");
    }
    sum += tx.amount;
    i += 1;
}
if total_amount != sum {
    panic!("total_amount must match sum of transaction amounts")
}
```

### 2. Added regression test (src/lib.rs)

**Location**: At the end of the tests module

**Change**: Add this test function:

```rust
#[test]
#[should_panic(expected = "total_amount must match sum of transaction amounts")]
fn test_finalize_settlement_panics_when_total_amount_mismatch() {
    let env = Env::default();
    let (admin, contract_id) = setup(&env);
    let client = SynapseContractClient::new(&env, &contract_id);
    let relayer = Address::generate(&env);
    let stellar = Address::generate(&env);
    let asset = SorobanString::from_str(&env, "USD");
    let anchor_id = SorobanString::from_str(&env, "total-mismatch");

    client.grant_relayer(&admin, &relayer);
    client.add_asset(&admin, &asset);
    let tx_id = client.register_deposit(&relayer, &anchor_id, &stellar, &100i128, &asset, &None);

    client.finalize_settlement(
        &relayer,
        &asset,
        &vec![&env, tx_id],
        &200i128, // Wrong total - should be 100
        &1u64,
        &2u64,
    );
}
```

## What this accomplishes

1. **Validation Logic**: The `finalize_settlement` function now calculates the sum of all transaction amounts and compares it with the provided `total_amount` parameter.

2. **Regression Test**: The test creates a transaction with amount 100, then calls `finalize_settlement` with a wrong total_amount of 200, expecting it to panic with the specific error message.

3. **Addresses Issue #36**: This provides the regression test requested for the total amount validation requirement.

## Files Modified
- `src/lib.rs` - Added validation logic and test

## Lines Changed
- Approximately 5 lines added for validation logic
- Approximately 25 lines added for the test
- Total: ~30 lines of focused changes

This is a minimal, focused commit that only addresses the specific requirement without introducing unrelated changes.