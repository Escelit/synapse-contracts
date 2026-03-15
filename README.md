# synapse-contract

Rust · WASM · Stellar/Soroban · Part of [synapse-core](../synapse-core)

---

Fiat hits the relayer. The contract takes over. State machine drives it to settlement, on-chain, in public, forever. No middleware eating your margin. No custodian holding your funds. No trust required — the ledger is the guarantee.

```
register_deposit
      │
   Pending ──► Processing ──► Completed
                    │
                 Failed ──► DLQ ──► retry_dlq ──► Processing
```

Every hop is an immutable on-chain event. You can't hide it. You can't undo it. That's not a bug.

---

## Roles

Two. Hard-coded. No governance theatre.

| Role | Who | What they can do |
|---|---|---|
| `admin` | Deployer | Add/remove relayers, allowlist assets, drain DLQ |
| `relayer` | synapse-core backend | Register deposits, advance state, finalise settlements |

Call something you're not authorised for and the contract panics. That's the entire access policy.

---

## Code

```
src/
├── lib.rs          ← entry points
├── types/mod.rs    ← Transaction, Settlement, DlqEntry, TransactionStatus
├── storage/mod.rs  ← all ledger I/O, centralised, no exceptions
├── access/mod.rs   ← require_admin / require_relayer
└── events/mod.rs   ← on-chain audit trail
tests/
└── contract_test.rs
```

---

## Ship it

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked soroban-cli

cargo build --target wasm32-unknown-unknown --release
cargo test

soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/synapse_contract.wasm \
  --network testnet \
  --source <YOUR_SECRET_KEY>
```

---

## Before mainnet — these will hurt you

| Where | The problem |
|---|---|
| `lib.rs` | `initialize` has no re-entrancy guard — anyone can reinitialise the contract |
| `lib.rs` | `retry_dlq` is a stub — DLQ entries are stuck forever |
| `types/mod.rs` | `generate_id` entropy is weak — collisions under load are real |
| `storage/mod.rs` | No TTL bumps — persistent entries expire and vanish silently |
| `access/mod.rs` | No `pause` / `unpause` — zero kill switch if something goes sideways |
| `tests/` | Most tests are stubs — the coverage number is a lie |

Open an issue before you start. Don't step on each other.
