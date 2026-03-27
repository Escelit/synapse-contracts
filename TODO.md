# TODOs for feature/issue-61-storage-pause-check

## Plan Steps:
1. [x] Add pause::is_paused check to deposits::save, settlements::save, dlq::push in src/storage/mod.rs
2. [ ] Add/update tests in src/lib.rs or snapshots for pause enforcement in storage mutators
3. [ ] cargo test
4. [ ] git add . &amp;&amp; git commit -m "Add pause::is_paused checks to storage mutators (#61 #65)"
5. [ ] git push origin feature/issue-61-storage-pause-check
6. [ ] gh pr create --title "Add pause checks to storage mutators (#61 #65)" --body "Check pause::is_paused at top of deposits::save, settlements::save, dlq::push.

Defense in depth for storage layer.

Closes #65 #61" --base develop

Updated after each completed step.
