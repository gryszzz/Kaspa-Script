# October 2026 Compatibility Checkpoint

Implementation commit: `742e545`.
Review: https://github.com/gryszzz/Kaspa-Script/pull/2.

## Delivered

- Isolated native transaction companion targeting Rusty Kaspa v2.1.0 commit
  `01b532e8b553523216471682649693af92f0fd16`.
- Explicit TN10 intent, integer-sompi inputs/outputs/fee, compute budgets,
  storage mass, genesis/continuation bindings, and native RPC round trips.
- A spend-instantiation fix: non-empty verified bodies end in a successful
  stack result; empty bodies are rejected. Existing artifact fragments retain
  their meaning and target gates.
- Native positive/underpayment checks for KaspaScript and released Silverscript
  v1.0.0 commit `3ed973335b59269293564805cc2c58a14595ec03`.
- A signed offline genesis/continuation/exit fixture with invalid-payment and
  replay controls plus deterministic fixture rollback/reapply.
- CLI status updated to the dated October node/compiler and activation baseline,
  with production-mainnet readiness still blocked.
- CI for the pinned native integration, triggered by compiler/SDK changes.

## Verification

```bash
cargo fmt --all -- --check
cargo test --workspace --features testnet-integration --locked
cargo clippy --workspace --all-targets --features testnet-integration --locked -- -D warnings
cargo fmt --manifest-path compatibility/toccata-v2/Cargo.toml -- --check
cargo test --manifest-path compatibility/toccata-v2/Cargo.toml --locked --features silverscript
cargo clippy --manifest-path compatibility/toccata-v2/Cargo.toml --locked --all-targets --features silverscript -- -D warnings
git diff --check
```

Workspace: 83 passed; two live tests ignored. Native companion: seven passed.
Both Clippy passes succeeded locally on arm64 macOS. GitHub CI provides a
separate Linux check; its status is visible on the PR.

## Remaining Boundaries

No live funded TN10 lifecycle was executed. The native library does not connect,
sign, or broadcast. A live run still needs a scoped funded testnet wallet,
verified node identity, wallet/node mass and fee calculation, submission and
acceptance evidence, and DAG/indexer reconciliation. No private key was read
from user configuration or sent anywhere during this pass.

The local fixture ledger is not a DAG indexer. Native engine validation is not
mempool/fee-policy or consensus acceptance. The legacy SDK's 0.15.0 lane remains
historical. Neither protocol activation nor these tests unlock production
contract lowering or establish wallet/indexer readiness.

The agent skill update is reviewed separately at
https://github.com/gryszzz/Kaspa-Ai-Agent-Skill/pull/23. Both installed skill
locations were backed up and synchronized with the tested context bundle.
