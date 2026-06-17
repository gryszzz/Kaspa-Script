# Kaspa Toccata Upgrade Prep

Prepared: 2026-06-04. Updated: 2026-06-17.

This note is the working training brief for getting KaspaScript ready for the
current Kaspa upgrade cycle. It separates upstream facts from KaspaScript work
still needed.

## Current Upstream State

Primary sources checked:

- `kaspanet/rusty-kaspa` release `v2.0.0`, published 2026-06-05:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.0
- `kaspanet/rusty-kaspa` release `v2.0.1`, published 2026-06-15:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1
- Tagged `v2.0.1` Toccata guide:
  https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md
- Baseline tagged `v2.0.0` Toccata guide:
  https://github.com/kaspanet/rusty-kaspa/blob/v2.0.0/docs/toccata-guide.md
- `kaspanet/rusty-kaspa` release `v1.3.0-toc.5`, published 2026-06-03:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v1.3.0-toc.5
- `kaspanet/rusty-kaspa` release `tn10-toc3`, published 2026-05-27:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/tn10-toc3
- `kaspanet/kips` `master` at commit
  `1aba3b8321c1d27e00b7d87bd7c74ef879efabdc`:
  https://github.com/kaspanet/kips

Important checkpoint: `v2.0.1` is the current mainnet Toccata maintenance
release and can be used as the upgrade version for pre-Toccata 1.x nodes.
`v2.0.0` remains the baseline release whose notes schedule activation at DAA
score `474,165,565`, roughly 2026-06-30 16:15 UTC. Until activation is
independently verified, KaspaScript treats both tags as mainnet
pre-activation evidence.

Follow-up source watch through 2026-06-17:

- `kaspanet/rusty-kaspa` published `v2.0.0` after the earlier moving-master
  watch, replacing the open activation-PR signal with a tagged release and
  scheduled activation DAA.
- `kaspanet/rusty-kaspa` published `v2.0.1` on 2026-06-15 with seq-commit
  lane-proof RPC support, SMT sync notifications, SMT inspection tooling,
  user-lane transaction generation, covenant-binding refinements, and a Wasm
  transaction v0 deserialization fix.
- Post-release changes touched wallet covenant bindings, txscript WASM builder
  flags, RPC transaction JSON requirements, `storage_mass`, `compute_commit`,
  TN10 reenablement, and mempool fee/relay behavior.

See [`RUSTY_KASPA_UPSTREAM_WATCH.md`](RUSTY_KASPA_UPSTREAM_WATCH.md) for the
moving-master architecture watch.

See [`TOCCATA_V2_INTEGRATION.md`](TOCCATA_V2_INTEGRATION.md) for the tagged
release integration brief now wired into the CLI status report.

## What Toccata Adds

| Area | Upstream source | Impact for KaspaScript |
| --- | --- | --- |
| ZK verification | KIP-16; `OpZkPrecompile` `0xa6`; Groth16 tag `0x20`; RISC0-Succinct tag `0x21` | `zk_verify` can become real only after stack ABI, proof artifacts, pricing assumptions, and fixtures are implemented. |
| Expanded covenants | KIP-17; transaction introspection, payload substrings, `OpCat`, `OpSubstr`, bitwise ops, keyed hashes, `OpBlake3`, signature-from-stack | The language can grow beyond KIP-10 input/output checks, but every opcode needs target gates and bytecode snapshots. |
| Covenant IDs | KIP-20; UTXO/output covenant ID model; `OpInputCovenantId`, `OpOutputCovenantId`, authorized-output context | `input(0).covenant_id` and `output(0).covenant_id` need distinct IR, plus transaction output covenant bindings in the SDK. |
| Partitioned sequencing | KIP-21; `OpChainblockSeqCommit` `0xd4` | `sequencing` needs a block-hash witness model, depth/reorg policy, and proof tests. |
| Fee policy | Tagged `v2.0.1` Toccata guide | Transaction submission must not rely on stale fixed-fee assumptions; fees should come from node APIs where possible. |
| Transaction shape | Tagged `v2.0.1` Toccata guide | Toccata v1 transactions add output covenant bindings and input compute commitments; builders must preserve both. |
| Seq-commit lane proof RPC | Rusty Kaspa `v2.0.1` | Sequencing readiness needs request/response fixtures before proof-bearing lowering is claimed. |
| Covenant binding refinements | Rusty Kaspa `v2.0.1` | SDK transaction facade should isolate upstream representation changes behind local package fields. |
| Node DB upgrade | Tagged `v2.0.1` Toccata guide | Upgraded node databases cannot be downgraded without resyncing. |

## Local Readiness Snapshot

- The local compiler still emits only the verified KIP-10-era subset.
- `kaspascript-kernel` now provides the first framework layer for
  Kaspa-native contract blueprints, wallet previews, covenant lineage schema,
  readiness reports, and Toccata fee-policy math.
- `kaspascript kernel package <contract.ks>` now emits v0 bytecode plus kernel
  package JSON: schema version, source snapshots, wallet previews, indexer
  schema, fee estimate, and readiness report.
- `kaspascript toccata status --json` now emits a current `v2.0.1`
  integration profile plus the baseline `v2.0.0` activation release for
  release assets, node guide, fee policy, v1 transaction fields, KIP map,
  and integrator actions.
- The SDK testnet integration depends on Kaspa crates `0.15.0`; Toccata
  compatibility must now be respiked against the `v2.0.1` line.
- The first Toccata crate compatibility spike is recorded in
  [TOCCATA_CRATE_COMPATIBILITY.md](TOCCATA_CRATE_COMPATIBILITY.md).
- `docs/kaspa-source-audit.md` now records current upstream Toccata evidence,
  but bytecode emission for covenant IDs, ZK verification, and sequencing
  remains unsupported until implementation and live proof coverage exist.
- `toccata-preview` is safe as an analysis/warning target, not a production
  promise.

## Preparation Plan

1. Source pinning: vendor or pin the exact upstream files for current
   `v2.0.1`, baseline `v2.0.0`, historical `v1.3.0-toc.5`, and current
   `kaspanet/kips`, then make the audit generated or checkable.
2. Moving-master watch: add a non-blocking compatibility lane for current
   `kaspanet/rusty-kaspa` `master` so API drift is found early without
   destabilizing pinned release tests.
3. Dependency spike: test whether Kaspa crates from the Toccata line can be
   consumed directly or whether this repo needs git dependencies/facade types.
4. IR split: replace broad `CovenantId`, `ZkVerifyGroth16`,
   `ZkVerifyRiscZero`, and `SequencingCommitment` placeholders with ABI-shaped
   instructions.
5. Kernel integration: feed compiler artifacts into `kaspascript-kernel`
   packages so every contract ships with wallet preview, indexer schema, fee
   policy, and readiness metadata.
6. Backend gates: add opcode constants and emission only after each instruction
   has stack-order tests, ASM snapshots, and target-specific activation checks.
7. SDK transaction builder: add transaction version/covenant binding support,
   fee estimation from RPC, and no-broadcast dry runs against upgraded TN10.
8. Proof fixtures: generate minimal TN10 fixtures for covenant ID continuation,
   seq-commit lane-proof RPCs, `OpChainblockSeqCommit`, Groth16, and
   RISC0-Succinct.
9. Mainnet posture: keep `future-mainnet` locked until activation at DAA score
   `474,165,565` is independently verified from primary sources.

## First Compatibility Spikes

Run these before adding production lowering:

```bash
cargo test --workspace
cargo test --workspace --features testnet-integration
```

Then, on a disposable upgraded node only:

```bash
export KASPA_TARGET=tn10
export KASPA_RPC_URL=ws://127.0.0.1:17210
cargo test --workspace --features testnet-integration -- --ignored --nocapture
```

Use a fresh/sacrificial node DB for Toccata testing because the upstream
pre-release line warned that the database upgrade is one-way.
