# Implementation Plan: dlq-count

## Overview

Expose the existing `dlq::get_count` storage helper as a public `get_dlq_count` query on `SynapseContract`, then add tests. The counter logic in `dlq::push` and `dlq::remove` is already correct — only the public entry-point and tests are missing.

## Tasks

- [x] 1. Add `get_dlq_count` query to `SynapseContract`
  - In `src/lib.rs`, inside `#[contractimpl] impl SynapseContract`, add:
    ```rust
    pub fn get_dlq_count(env: Env) -> i128 {
        dlq::get_count(&env)
    }
    ```
  - No auth or pause check needed — read-only query.
  - Remove the `// TODO(#60)` comment from `src/storage/mod.rs` once the function is wired.
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [ ] 2. Write tests in `tests/contract_test.rs`
  - [-] 2.1 Test: `get_dlq_count_returns_zero_on_fresh_contract`
    - Call `get_dlq_count` on a freshly initialised contract with no DLQ activity.
    - Assert the return value is 0.
    - `// Feature: dlq-count, Property baseline example`
    - _Requirements: 3.2, 5.1_

  - [-] 2.2 Test: `get_dlq_count_increments_on_mark_failed`
    - Register a deposit, call `mark_failed`, assert `get_dlq_count` == 1.
    - Call `mark_failed` on a second deposit, assert `get_dlq_count` == 2.
    - `// Feature: dlq-count, Property A: push increments count by 1`
    - _Requirements: 1.1, 4.1, 5.2_

  - [-] 2.3 Test: `get_dlq_count_decrements_on_retry_dlq`
    - Push two entries via `mark_failed`, assert count == 2.
    - Call `retry_dlq` on one, assert count == 1.
    - `// Feature: dlq-count, Property B: remove decrements count by 1`
    - _Requirements: 2.1, 4.2, 5.3_

  - [-] 2.4 Test: `get_dlq_count_decrements_on_mark_completed`
    - Push an entry via `mark_failed`, assert count == 1.
    - Call `mark_processing` then `mark_completed` on the same tx, assert count == 0.
    - `// Feature: dlq-count, Property B (mark_completed path)`
    - _Requirements: 2.1, 4.3, 5.4_

  - [-] 2.5 Test: `get_dlq_count_never_goes_below_zero`
    - On a fresh contract (count == 0), call `retry_dlq` is not applicable without an entry.
    - Instead, push one entry, remove it (count == 0), then verify count stays 0 after a second remove attempt via a direct `mark_completed` on a tx that is no longer in the DLQ.
    - Assert `get_dlq_count` == 0 throughout.
    - `// Feature: dlq-count, edge-case: saturating subtraction`
    - _Requirements: 2.2, 5.5_

  - [-]* 2.6 Test: `get_dlq_count_round_trip_push_then_remove_all`
    - Push N entries via `mark_failed`, assert count == N.
    - Remove all N via `retry_dlq`, assert count == 0.
    - `// Feature: dlq-count, Property C: push N then remove N returns to 0`
    - _Requirements: 2.3, 3.3_

- [~] 3. Checkpoint — Ensure all tests pass
  - Run `cargo test` and confirm all new and existing tests pass. Ask the user if any questions arise.
