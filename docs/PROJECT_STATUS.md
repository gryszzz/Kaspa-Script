# Project Status And Completion Roadmap

Updated: 2026-06-05.

KaspaScript is now a source-gated compiler plus the first slice of a
Kaspa-native programmability kernel.

The project can compile the verified V1 KaspaScript subset, produce
deterministic txscript artifacts, and package a contract with kernel metadata:
capability profile, wallet previews, indexer schema, readiness report,
bytecode ASM/hex, and a Toccata fee estimate.

## What Works Today

Compiler:

- Lexer, parser, semantic checks, typed IR, and deterministic backend emission.
- Golden artifacts for the verified example contracts.
- `verified-tn12`, `tn10-toccata`, `toccata-preview`, and `future-mainnet`
  target posture.
- Source-grounded warnings for gated covenant, ZK, and sequencing surfaces.

CLI:

- `kaspascript compile <contract.ks>`
- `kaspascript inspect <contract.ks>`
- `kaspascript verify <artifact.json>`
- `kaspascript kernel package <contract.ks> --target <target>`

Kernel:

- `kaspascript-kernel` workspace crate.
- `DAGSafeVault` UTXO state-machine blueprint.
- Compiled kernel package JSON.
- Kernel Package v0 schema version.
- Golden package snapshots for `escrow.ks` and `vault.ks`.
- Capability profile with execution model, feature evidence, transition
  profiles, wallet requirements, indexer requirements, and policy limits.
- Wallet preview metadata.
- Covenant lineage indexer schema.
- Readiness report with mainnet activation guard.
- Human-readable kernel package schema reference.
- Toccata pre-activation fee policy:
  `100 sompi * max(compute grams, 2 * transaction bytes)`.

Testnet readiness:

- Feature-gated TN12/testnet harness.
- Offline deployment previews.
- Toccata source and crate compatibility notes.
- Rusty Kaspa upstream watch for moving-master architecture changes.
- Rusty Kaspa `v2.0.0` is tracked as mainnet pre-activation evidence; its
  release notes schedule activation at DAA score `474,165,565`, roughly
  June 30, 2026 at 16:15 UTC.

## What This Does Not Claim Yet

- It does not claim Toccata mainnet activation.
- It does not claim the June 30, 2026 scheduled Toccata activation has occurred.
- It does not emit production covenant ID, ZK verifier, or script-visible
  sequencing bytecode yet.
- It does not build or broadcast real covenant-bearing Toccata transactions.
- It does not replace wallet, node, or indexer verification.

## Definition Of Complete

### Alpha Complete

The current milestone is close to alpha complete. It needs:

- README examples for compile, inspect, verify, and kernel package.
- CI running `cargo fmt --check`, `cargo clippy`, and workspace tests.

### Testnet Complete

The testnet milestone needs:

- Toccata git-tag compatibility fixture in CI or a documented local check.
- TN10-compatible transaction builder facade.
- Covenant ID continuation fixtures.
- Wallet preview golden cases for each production contract pattern.
- Indexer fixtures for genesis, continuation, duplicate transition, reorg, and
  wrong-network cases.
- Optional live TN10 no-broadcast dry-run path.

### Mainnet Ready

Mainnet readiness is blocked until primary sources verify:

- activation occurred at the scheduled DAA score
- pinned Rusty Kaspa crate/tag compatibility
- wallet/indexer support assumptions
- final fee/mass behavior

Until those exist, `future-mainnet` remains locked.

## Immediate Next Tasks

1. Add constraint extraction for signatures, timelocks, value conservation, and
   script binding in the capability profile.
2. Add a machine-readable JSON Schema for `kaspascript.kernel.package.v0`.
3. Update CI to run format, clippy, workspace tests, and kernel package golden
   checks.
4. Add a small SDK wrapper for generating kernel packages without invoking the
   CLI.
5. Add TN10-oriented package fixtures for Toccata readiness.
6. Add wallet-preview and indexer fixtures for each production contract pattern.

## Maintainer Commands

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --features testnet-integration
```

Smoke test the kernel package command:

```bash
cargo run -p kaspascript-cli -- kernel package tests/contracts/escrow.ks \
  --output /tmp/escrow.kernel.json \
  --compute-grams 1000 \
  --tx-bytes 400
```
