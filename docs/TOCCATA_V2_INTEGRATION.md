# Toccata v2.0.x Integration Brief

Prepared: 2026-06-05. Updated: 2026-06-17.

This is the KaspaScript source-grounded integration brief for Rusty Kaspa
`v2.0.1`, the current mainnet Toccata maintenance release, with `v2.0.0`
preserved as the baseline release that scheduled activation.

Primary sources:

- Rusty Kaspa release:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.1
- Baseline Rusty Kaspa release:
  https://github.com/kaspanet/rusty-kaspa/releases/tag/v2.0.0
- Tagged Toccata guide:
  https://github.com/kaspanet/rusty-kaspa/blob/v2.0.1/docs/toccata-guide.md
- Baseline tagged Toccata guide:
  https://github.com/kaspanet/rusty-kaspa/blob/v2.0.0/docs/toccata-guide.md
- KIP-16:
  https://github.com/kaspanet/kips/blob/master/kip-0016.md
- KIP-17:
  https://github.com/kaspanet/kips/blob/master/kip-0017.md
- KIP-20:
  https://github.com/kaspanet/kips/blob/master/kip-0020.md
- KIP-21:
  https://github.com/kaspanet/kips/blob/master/kip-0021.md

## Status

`v2.0.1` is the current mainnet Toccata maintenance release. `v2.0.0` is the
baseline release that schedules hardfork activation at DAA score
`474,165,565`, roughly 2026-06-30 16:15 UTC.

KaspaScript therefore treats `v2.0.1` as:

- current upstream release evidence
- a drop-in upgrade version for `v2.0.0` nodes and pre-Toccata 1.x nodes
- mainnet pre-activation evidence
- not proof that production mainnet KaspaScript contracts are active

The `future-mainnet` target remains blocked until activation is independently
verified from primary sources.

## What Changed For KaspaScript

| Area | v2.0.x signal | KaspaScript integration |
| --- | --- | --- |
| Node posture | Node operators should upgrade before activation; node DB upgrade is one-way. | Keep package reports explicit about activation guard and node upgrade assumptions. |
| P2P | Nodes require P2P protocol version 10 peers starting 24 hours before activation. | Surface protocol requirement in `kaspascript toccata status --json`. |
| Fees | Minimum standard fee policy is `100 sompi * max(compute grams, 2 * transaction bytes)`. | Point kernel fee estimates at the tagged v2.0.1 guide. |
| Transaction shape | Toccata v1 transactions add `TransactionOutput.covenant` and `TransactionInput.compute_commit`. | Add transaction-format metadata to CLI reports; do not lower covenant transactions until builder support lands. |
| Mass fields | New Rust/protobuf integrations should prefer `storage_mass`; JSON/JS should prefer `storageMass`. | Avoid new `mass`-based SDK surfaces except compatibility reads. |
| Mining/pools | Block template flow must preserve covenant and compute-commit fields through submit. | Treat miner/pool compatibility as a readiness checklist item, not a compiler feature. |
| TN10 rehearsal | Wallets, explorers, pools, miners, exchanges, and indexers should rehearse on Testnet-10. | Keep `tn10-toccata` as the Toccata design and compatibility target. |
| Sequencing RPC | `v2.0.1` adds seq-commit state and lane-proof RPC support. | Add fixtures for lane-proof payloads and keep sequencing proof support gated. |
| Covenant bindings | `v2.0.1` refines covenant-binding handling across client and wallet code. | Keep SDK covenant-binding metadata behind local facade types until builder support is pinned. |

## KIP Surface

KIP-16 adds `OpZkPrecompile` (`0xa6`) for verifiable computation. Initial
precompile tags are Groth16 (`0x20`) and RISC0-Succinct (`0x21`). KaspaScript
should only lower this after proof ABI fixtures, verifier payload rules, and
pricing assumptions are pinned.

KIP-17 expands covenant scripting with transaction introspection, byte-string
operators, signature-from-stack opcodes, keyed hashes, BLAKE3, and larger
post-activation script limits. KaspaScript can model these in IR before it
emits production Toccata bytecode.

KIP-20 adds covenant IDs and transaction output covenant bindings. This is the
native lineage primitive KaspaScript packages already prepare for with indexer
schema and wallet previews.

KIP-21 replaces monolithic sequencing commitment proofs with partitioned lane
commitments intended to make proving scale with lane activity. KaspaScript
should treat `OpChainblockSeqCommit` as a proof-bearing transition primitive
that needs lane witness metadata.

## Repo Integration

`kaspascript toccata status --json` now includes:

- current `v2.0.1` release metadata and assets
- baseline `v2.0.0` activation release metadata
- tagged node upgrade guide metadata
- activation DAA and P2P version guard
- minimum and preferred node requirements
- Toccata fee policy semantics
- v1 transaction field changes
- KIP-to-KaspaScript integration map
- integrator actions for wallets, CI, agents, miners, pools, and indexers

The payload is published under
`kaspascript.cli.toccata.status.v0` and described by
[`schemas/kaspascript.cli.toccata.status.v0.schema.json`](schemas/kaspascript.cli.toccata.status.v0.schema.json).

## Next Engineering Moves

1. Add a non-default Toccata git-dependency compatibility feature pinned to
   Rusty Kaspa `v2.0.1`.
2. Build a covenant transaction builder facade that knows transaction version
   `1`, covenant bindings, `compute_commit`, and `storage_mass`.
3. Add fixtures for seq-commit lane-proof RPCs and covenant-binding
   representations added/refined in `v2.0.1`.
4. Add IR nodes for covenant IDs, authorized outputs, output covenant binding
   inspection, and sequencing lane witnesses.
5. Add opcode ABI snapshots from Rusty Kaspa `crypto/txscript` before enabling
   lowering.
6. Add TN10 integration fixtures for covenant genesis, continuation,
   fee-policy rejection, and wallet preview signing flows.
