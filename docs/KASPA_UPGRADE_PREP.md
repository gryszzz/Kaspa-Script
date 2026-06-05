# Kaspa Toccata Upgrade Prep

Prepared: 2026-06-04.

This note is the working training brief for getting KaspaScript ready for the
current Kaspa upgrade cycle. It separates upstream facts from KaspaScript work
still needed.

## Current Upstream State

Primary sources checked:

- `kaspanet/rusty-kaspa` release `v1.3.0-toc.5`, published 2026-06-03:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v1.3.0-toc.5
- `kaspanet/rusty-kaspa` release `tn10-toc3`, published 2026-05-27:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/tn10-toc3
- `kaspanet/kips` `master` at commit
  `1aba3b8321c1d27e00b7d87bd7c74ef879efabdc`:
  https://github.com/kaspanet/kips

Important checkpoint: `v1.3.0-toc.5` is a mainnet pre-activation
pre-release. It is for sanity testing and does not activate Toccata on mainnet.
Operators should expect another upgrade for the final rollout.

## What Toccata Adds

| Area | Upstream source | Impact for KaspaScript |
| --- | --- | --- |
| ZK verification | KIP-16; `OpZkPrecompile` `0xa6`; Groth16 tag `0x20`; RISC0-Succinct tag `0x21` | `zk_verify` can become real only after stack ABI, proof artifacts, pricing assumptions, and fixtures are implemented. |
| Expanded covenants | KIP-17; transaction introspection, payload substrings, `OpCat`, `OpSubstr`, bitwise ops, keyed hashes, `OpBlake3`, signature-from-stack | The language can grow beyond KIP-10 input/output checks, but every opcode needs target gates and bytecode snapshots. |
| Covenant IDs | KIP-20; UTXO/output covenant ID model; `OpInputCovenantId`, `OpOutputCovenantId`, authorized-output context | `input(0).covenant_id` and `output(0).covenant_id` need distinct IR, plus transaction output covenant bindings in the SDK. |
| Partitioned sequencing | KIP-21; `OpChainblockSeqCommit` `0xd4` | `sequencing` needs a block-hash witness model, depth/reorg policy, and proof tests. |
| Fee policy | `v1.3.0-toc.5` release notes | Transaction submission must not rely on stale fixed-fee assumptions; fees should come from node APIs where possible. |
| Node DB upgrade | `v1.3.0-toc.5` release notes | Dev/test nodes upgraded to this pre-release cannot be downgraded without resyncing. |

## Local Readiness Snapshot

- The local compiler still emits only the verified KIP-10-era subset.
- `kaspascript-kernel` now provides the first framework layer for
  Kaspa-native contract blueprints, wallet previews, covenant lineage schema,
  readiness reports, and Toccata fee-policy math.
- `kaspascript kernel package <contract.ks>` now emits bytecode plus kernel
  package JSON: wallet previews, indexer schema, fee estimate, and readiness
  report.
- The SDK testnet integration depends on Kaspa crates `0.15.0`; current
  Toccata pre-release artifacts are in the `v1.3.0-toc.5` line.
- The first Toccata crate compatibility spike is recorded in
  [TOCCATA_CRATE_COMPATIBILITY.md](TOCCATA_CRATE_COMPATIBILITY.md).
- `docs/kaspa-source-audit.md` now records current upstream Toccata evidence,
  but bytecode emission for covenant IDs, ZK verification, and sequencing
  remains unsupported until implementation and live proof coverage exist.
- `toccata-preview` is safe as an analysis/warning target, not a production
  promise.

## Preparation Plan

1. Source pinning: vendor or pin the exact upstream files for
   `v1.3.0-toc.5` and current `kaspanet/kips`, then make the audit generated or
   checkable.
2. Dependency spike: test whether Kaspa crates from the Toccata line can be
   consumed directly or whether this repo needs git dependencies/facade types.
3. IR split: replace broad `CovenantId`, `ZkVerifyGroth16`,
   `ZkVerifyRiscZero`, and `SequencingCommitment` placeholders with ABI-shaped
   instructions.
4. Kernel integration: feed compiler artifacts into `kaspascript-kernel`
   packages so every contract ships with wallet preview, indexer schema, fee
   policy, and readiness metadata.
5. Backend gates: add opcode constants and emission only after each instruction
   has stack-order tests, ASM snapshots, and target-specific activation checks.
6. SDK transaction builder: add transaction version/covenant binding support,
   fee estimation from RPC, and no-broadcast dry runs against upgraded TN10.
7. Proof fixtures: generate minimal TN10 fixtures for covenant ID continuation,
   `OpChainblockSeqCommit`, Groth16, and RISC0-Succinct.
8. Mainnet posture: keep `future-mainnet` locked until the final mainnet
   activation release and DAA score are pinned from primary sources.

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

Use a fresh/sacrificial node DB for `v1.3.0-toc.5` testing because the upstream
release warns that the database upgrade is one-way.
