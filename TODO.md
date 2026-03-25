# Issue #31: Cap max retry_count; emit MaxRetriesExceeded

## Original Steps (Completed)
- [x] 1. Create/switch to branch `feature/issue-31-max-retries`
- [x] 2. Edit `src/types/mod.rs`: Add MAX_RETRIES const, Event::DlqRetried & MaxRetriesExceeded variants
- [x] 3. Edit `src/lib.rs`: Implement retry_dlq logic (check/increment retry_count, reset tx or emit+panic)
- [x] 4. Edit `tests/contract_test.rs`: Update existing retry test, add retry/max retries tests

## Merge Conflict Resolution (New)
- [ ] 5. Resolve conflicts in src/types/mod.rs (clean Event enum + generate_id)
- [ ] 6. Resolve conflicts in tests/contract_test.rs (add clean DLQ tests, remove outdated TODOs)
- [ ] 7. Update this TODO.md
- [ ] 8. Run `cargo check && cargo test`
- [ ] 9. `git add . && git commit -m "Resolve merge conflicts for #31"`

blow
