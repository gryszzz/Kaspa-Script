# Rusty Kaspa Upstream Watch

Updated: 2026-06-17.

This brief tracks `kaspanet/rusty-kaspa` as the primary upstream source for
KaspaScript architecture, compatibility, and Toccata readiness.

The goal is not to copy Rusty Kaspa. The goal is to learn from the reference
implementation like a compiler engineer: identify consensus-facing shape
changes, wallet/RPC serialization changes, fee and mass policy changes,
txscript builder changes, and activation evidence.

## Current Upstream Snapshot

Source repository:
[`kaspanet/rusty-kaspa`](https://github.com/kaspanet/rusty-kaspa).

Tagged Toccata guide:
[`docs/toccata-guide.md` at `v2.0.1`](https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md).
The baseline `v2.0.0` guide remains useful for comparing the original
activation release.

As of this audit:

- default branch: `master`
- latest release:
  [`v2.0.1`](https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1),
  published 2026-06-15
- `v2.0.1` tag commit:
  [`cfafeb4c093f`](https://github.com/kaspanet/rusty-kaspa/commit/cfafeb4c093fa37a303f1b9f19c58f986b870ce3)
- baseline Toccata release:
  [`v2.0.0`](https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.0),
  published 2026-06-05
- `v2.0.0` tag commit:
  [`90dbf074275d`](https://github.com/kaspanet/rusty-kaspa/commit/90dbf074275d60c1fe74a3491883196f110970c0)
- latest stable branch observed: `stable` at the `v1.1.0` line
- activation schedule from the release notes: DAA score `474,165,565`,
  roughly 2026-06-30 16:15 UTC

`v2.0.1` is the current mainnet Toccata maintenance release and can be used as
the upgrade version for pre-Toccata 1.x nodes. `v2.0.0` remains the baseline
release that scheduled activation at DAA score `474,165,565`. At this audit
date activation is still scheduled in the future, so KaspaScript treats both
release tags as mainnet pre-activation evidence, not as proof that mainnet
contract support is active.

## Why This Matters

Rusty Kaspa moved quickly after `v1.3.0-toc.5`. `master` was already 10 commits
ahead of the tag during this audit.

That means KaspaScript needs three upstream lanes:

- release lane: reproducible compatibility against current `v2.0.1` while
  keeping `v2.0.0` as the baseline activation release
- legacy pinned lane: comparison against `v1.3.0-toc.5`
- moving master lane: early warning for API, RPC, wallet, and txscript changes

The release lane protects tests. The moving lane trains the architecture.

## Post-Release Delta Since `v1.3.0-toc.5`

| Upstream commit | Change | Architecture signal for KaspaScript |
| --- | --- | --- |
| [`d5205cc72ab7`](https://github.com/kaspanet/rusty-kaspa/commit/d5205cc72ab7b811e88a23595dfac5b9facdeece) | Docker Alpine Rust version bumped to 1.91. | Keep Toccata compatibility checks on a Rust toolchain new enough for the upstream workspace. |
| [`ae51b8a5072e`](https://github.com/kaspanet/rusty-kaspa/commit/ae51b8a5072ed42984b58d32032569be9d2f7d22) | TN10 reenabled. | Add a TN10 lane for Toccata package/readiness checks instead of treating TN12 as the only live testnet posture. |
| [`770e3e9d4fd2`](https://github.com/kaspanet/rusty-kaspa/commit/770e3e9d4fd29e56869646193308bf39aeeac3e2) | RPC can get block reward by hash. | Keep RPC models flexible; reward lookups may become useful for fee/readiness reports. |
| [`a9451167d721`](https://github.com/kaspanet/rusty-kaspa/commit/a9451167d721fd9760582eedf34cec7b51c4f36a) | `tx.mass` renamed to `tx.storage_mass`. | Do not build new JSON/RPC code around `tx.mass`; prefer explicit `storage_mass`. |
| [`c26d517a80aa`](https://github.com/kaspanet/rusty-kaspa/commit/c26d517a80aaaf52b80cbb426355abbae3a470b6) | `input.mass` renamed to `input.compute_commit`; `TxInputMass` renamed to `ComputeCommit`. | Track upstream vocabulary carefully: transaction byte/mass fee estimates and input compute commitments are different concepts. |
| [`9bd6581b9c25`](https://github.com/kaspanet/rusty-kaspa/commit/9bd6581b9c25cb0940856d136924b1c644e4042e) | `storage_mass` became required when decoding `RpcTransaction` JSON. | Future SDK/RPC JSON schema should require `storage_mass` once it consumes current Toccata RPC objects. |
| [`36126503b812`](https://github.com/kaspanet/rusty-kaspa/commit/36126503b812a5ea2e1a673cd3beea111a715e35) | Removed consensus current-block-color getter. | Avoid depending on transient consensus helper APIs unless they are proven stable. |
| [`c1d8189303cd`](https://github.com/kaspanet/rusty-kaspa/commit/c1d8189303cd9cacbb39ef326ac4aa23a5971a70) | WASM txscript builder now allows passing script builder flags. | KaspaScript WASM/SDK bindings should expose builder flags rather than hiding them. |
| [`bbadf5e57170`](https://github.com/kaspanet/rusty-kaspa/commit/bbadf5e5717042ad30634f389505a3e2f8b6902a) | Wallet generator includes covenant bindings. | Contract transaction builders must preserve covenant bindings; wallet preview metadata should eventually show them. |
| [`580fa8b5d5a6`](https://github.com/kaspanet/rusty-kaspa/commit/580fa8b5d5a66b55db368cd47781784b8b631222) | WASM mempool entries request args are required. | Do not assume empty RPC/WASM request objects are accepted; package clients should send explicit args. |

## Maintenance Delta Since `v2.0.0`

| Upstream commit | Change | Architecture signal for KaspaScript |
| --- | --- | --- |
| [`2787953efb84`](https://github.com/kaspanet/rusty-kaspa/commit/2787953efb84d58f5cdda282953e60bea7da253b) | Added seq-commit lane proof RPC over gRPC and wRPC. | Add lane-proof request/response fixtures and expose sequencing proof readiness without enabling production lowering. |
| [`c53a83bf482`](https://github.com/kaspanet/rusty-kaspa/commit/c53a83bf4829d430d5179fd02f6d756dab782028) | Changed consensus-client covenant binding inner type. | Keep covenant-binding metadata behind facade types so SDK callers do not depend on unstable upstream representation. |
| [`b2d8759c540`](https://github.com/kaspanet/rusty-kaspa/commit/b2d8759c5408cdea9c6ce4306d0285e4cdafa0d2) | Added transaction generation targeting user lanes. | Extend no-broadcast dry-run design to preserve lane target metadata in previews. |
| [`cfafeb4c093`](https://github.com/kaspanet/rusty-kaspa/commit/cfafeb4c093fa37a303f1b9f19c58f986b870ce3) | Fixed Wasm client transaction v0 deserialization. | Add compatibility checks that legacy transaction decoding still works around Toccata clients. |

## Open PRs To Watch

| PR | Why it matters |
| --- | --- |
| [`#1025 remove TRANSIENT_BYTE_TO_MASS_FACTOR`](https://github.com/kaspanet/rusty-kaspa/pull/1025) | Mass/fee terminology is still being cleaned up in the Toccata lane. |
| [`#953 Zk sdk`](https://github.com/kaspanet/rusty-kaspa/pull/953) | ZK SDK shape may determine how KaspaScript packages proof hints and verifier payloads. |
| [`#991 UtxoIndex keyed by DAA score`](https://github.com/kaspanet/rusty-kaspa/pull/991) | DAA-indexed UTXO queries could help contract indexer fixtures and readiness reports. |

## Architecture Map To Study

| Upstream area | What to learn | KaspaScript surface |
| --- | --- | --- |
| `consensus/core` | Transaction shape, hashing, sighash, mass, serialization, activation params. | artifact verification, SDK transaction builder, readiness gating |
| `crypto/txscript` | Opcode definitions, script builder flags, pricing, WASM txscript bindings. | compiler backend, bytecode ASM/hex, WASM SDK |
| `wallet/core` | Transaction generation, mass calculation, covenant bindings. | wallet preview, package generation, testnet transaction facade |
| `wallet/pskt` | PSKT conversion and covenant-bearing transaction fields. | future contract package signing flow |
| `rpc/core`, `rpc/grpc`, `rpc/wrpc` | RPC transaction JSON, gRPC/wRPC models, required fields. | SDK schema, indexer integration, live fee estimates |
| `mining/mempool` | Standardness, relay, fee policy, transient/storage mass behavior. | fee estimates and readiness reports |
| `consensus/seq-commit`, `consensus/smt-store`, `crypto/smt` | Sequencing, SMT, and proof support. | DAGSafeVault, proof-bearing transitions, indexer lineage |

## KaspaScript Response Plan

1. Keep `future-mainnet` locked until activation at DAA score `474,165,565` is
   independently verified.
2. Move Toccata crate compatibility work to `v2.0.1`, while keeping
   `v2.0.0` as the baseline activation release and `v1.3.0-toc.5` as a
   historical comparison point.
3. Add TN10-oriented readiness fixtures for `tn10-toccata`.
4. Keep the CLI Toccata status schema as the machine-readable digest of the
   current tagged release.
5. Track `storage_mass`, `compute_commit`, and covenant bindings explicitly in
   the SDK transaction builder plan.
6. Add wallet-preview fields for covenant bindings once the transaction builder
   can construct them.
7. Add compatibility fixtures for seq-commit lane-proof RPCs, covenant-binding
   representations, and legacy transaction v0 deserialization.
8. Extend source snapshot metadata with upstream branch watches once the moving
   master lane is automated.

## Training Loop

Run this fast watch when upstream moves:

```bash
gh api repos/kaspanet/rusty-kaspa --jq '{default_branch,pushed_at,html_url}'
gh api 'repos/kaspanet/rusty-kaspa/releases?per_page=5' \
  --jq 'map({tag_name,prerelease,published_at,html_url})'
gh api 'repos/kaspanet/rusty-kaspa/compare/v1.3.0-toc.5...master' \
  --jq '{status,ahead_by,total_commits,commits:[.commits[] | {sha:.sha[0:12],message:(.commit.message|split("\n")[0]),html_url}]}'
gh api 'repos/kaspanet/rusty-kaspa/pulls?state=open&sort=updated&direction=desc&per_page=10' \
  --jq 'map({number,title,updated_at,base:.base.ref,head:.head.ref,html_url})'
```

Weekly deep dive:

1. Read `consensus/core/src/tx.rs`, `consensus/core/src/mass`, and transaction
   serde changes.
2. Read `crypto/txscript/src/lib.rs` and `crypto/txscript/src/script_builder.rs`.
3. Read `wallet/core/src/tx/generator/generator.rs`.
4. Read `rpc/core/src/model/tx.rs` and conversion code.
5. Convert findings into KaspaScript fixtures, not claims.
