<p align="center">
  <img src="./logo.png" alt="Kaspa AI Agent Skill Logo" width="200">
</p>

<h1 align="center">Kaspa Script </h1>
> Production-grade contract compiler architecture for Kaspa's covenant era.

![Rust](https://img.shields.io/badge/Rust-1.80+-111111?style=flat-square&logo=rust)
![Deterministic Builds](https://img.shields.io/badge/Deterministic-Builds-111111?style=flat-square)
![TN12 Target](https://img.shields.io/badge/Target-TN12-111111?style=flat-square)
![Tests Passing](https://img.shields.io/badge/Tests-Passing-111111?style=flat-square)
![LLVM-style Architecture](https://img.shields.io/badge/LLVM--style-Architecture-111111?style=flat-square)
![Source-Grounded](https://img.shields.io/badge/Protocol-Source--Grounded-111111?style=flat-square)

KaspaScript is a Rust compiler workspace for deterministic Kaspa contract
artifacts. It exists to make covenant-era contract construction inspectable:
source becomes typed IR, IR becomes target-gated txscript bytes, and every
protocol-sensitive claim is pinned to a source.

The project is built for reviewers, tooling, and coding agents that need stable
compiler surfaces: golden artifacts, explicit target gates, bytecode
verification, and no hidden transaction behavior.

Current status:

- Verified compiler and golden test suite for the V1 txscript subset.
- First Kaspa programmability kernel crate and CLI package command.
- Toccata upgrade prep is tracked, but mainnet activation remains unclaimed.

Read the status roadmap: [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md).
Kernel package schema:
[`docs/KERNEL_PACKAGE_SCHEMA.md`](docs/KERNEL_PACKAGE_SCHEMA.md).

---

## Architecture

```text
┌───────────┐    ┌────────┐    ┌────────┐    ┌─────┐
│ Source.ks │ -> │ Lexer  │ -> │ Parser │ -> │ AST │
└───────────┘    └────────┘    └────────┘    └─────┘
                                                   │
                                                   v
┌──────────┐    ┌────────────────┐    ┌───────────┐
│ Artifact │ <- │ Kaspa txscript │ <- │ Typed IR  │
└──────────┘    │ backend/gates  │    └───────────┘
                └────────────────┘          ^
                        ^                    │
                        │             ┌──────────────┐
                        └──────────── │ Semantics    │
                                      └──────────────┘

┌────────────────────────────────────────────────────────┐
│ Programmability Kernel                                 │
│ contract blueprints, wallet previews, indexer schema,  │
│ source evidence, Toccata fee policy, readiness reports │
└────────────────────────────────────────────────────────┘

Optimization passes are planned; today the compiler favors verifiable lowering
and deterministic emission over speculative transformation.
```

| Stage | Role |
| --- | --- |
| Lexer | Position-tagged token stream with line, column, and byte spans. |
| Parser | Contract AST for params, spend paths, calls, arrays, fields, and expressions. |
| Semantic Analysis | Collects type, scope, finality, builtin, and target-safety errors. |
| Typed IR | Opcode-agnostic instruction layer for contract verification and backend selection. |
| Backend Gates | Emits only source-grounded txscript for `verified-tn12`; gates preview surfaces. |
| Artifact | Deterministic JSON containing bytecode, source hash, target, KIP requirements, and warnings. |
| Kernel | Packages Kaspa-native contract blueprints with wallet previews, covenant lineage schema, fee policy, and network readiness. |

---

## Example Contract

```kaspascript
contract Escrow {
  params {
    buyer: PublicKey,
    seller: PublicKey,
    arbiter: PublicKey,
    timeout: BlockHeight,
    finality_depth: 10,
  }

  spend release(sig_a: Signature, sig_b: Signature) {
    require multisig(2, [buyer, seller, arbiter], [sig_a, sig_b]);
    require output(0).value >= input(0).value;
  }

  spend refund(sig: Signature) {
    require sig.verify(buyer);
    require block.height >= timeout;
    require output(0).script == buyer;
  }
}
```

Artifact metadata:

```json
{
  "backend": "kaspa-txscript",
  "target": "verified-tn12",
  "compiler_version": "0.1.0",
  "finality_depth": 10,
  "kip_requirements": [10],
  "warnings": []
}
```

IR preview:

```text
IR contracts: 1
contract Escrow
  spend release: 10 instructions
  spend refund: 9 instructions
```

Compile output:

```console
$ kaspascript compile escrow.ks
escrow.artifact.json
```

---

## Compiler Philosophy

KaspaScript treats compilation as an auditable system boundary.

| Principle | Meaning |
| --- | --- |
| Deterministic artifacts | The same source must produce the same bytecode and source hash every time. |
| No hidden behavior | The compiler and SDK do not inject invisible fees, treasury outputs, or implicit spend paths. |
| Source-grounded protocol support | Backend opcodes and KIP claims must cite pinned Kaspa sources before they can be verified. |
| Upgrade-safe targets | `verified-tn12`, `toccata-preview`, and `future-mainnet` are separate target gates. |
| Verification first | Unsupported behavior fails before bytecode emission; preview behavior must warn explicitly. |

The compiler refuses to guess. Uncertain protocol support is a gate, not a
branch.

---

## Features

### Implemented

| Area | Status |
| --- | --- |
| Lexer, parser, AST | Complete V1 front end with source positions. |
| Semantic analysis | Collects all errors instead of stopping at the first failure. |
| Typed IR | Opcode-agnostic lowering for verified V1 patterns. |
| Kaspa txscript backend | Emits deterministic bytes for source-grounded opcodes. |
| CLI | `compile`, `inspect`, `verify`, and `kernel package`. |
| Golden artifacts | JSON, hex, and ASM snapshots for every example contract. |
| SDK preview model | Compile API plus finality-depth checks; not a real Kaspa transaction builder yet. |
| TN12 test harness | Feature-gated live RPC/wallet preflight with gated proof files. |
| Programmability kernel | `kaspascript-kernel` crate plus `kaspascript kernel package <contract.ks>` for bytecode, wallet previews, indexer schema, readiness, and fee estimates. |

### Verified

| Surface | Evidence |
| --- | --- |
| Base txscript opcodes | Pinned `rusty-kaspa` txscript sources. |
| Canonical pushes | Pinned `rusty-kaspa` script builder behavior. |
| KIP-10 introspection | `input` / `output` value and script opcodes. |
| KIP-15 sequencing | Verified as a block-header commitment, not a script opcode. |

### Target-Gated

| Surface | Gate |
| --- | --- |
| `block.height` template values | Verified opcode, but transaction instantiation is still preview. |
| Covenant IDs | No pinned txscript opcode yet. |
| ZK verification | No pinned txscript verifier opcode yet. |
| Script-level sequencing access | KIP-15 is not script-visible in pinned sources. |
| Future mainnet target | Locked until mainnet sources are pinned. |

### Planned

| Area | Direction |
| --- | --- |
| Optimization passes | Deterministic IR transforms after verification invariants are fixed. |
| Real transaction builder | Rusty Kaspa transaction construction and submission. |
| WASM SDK | Stable compiler bindings for TypeScript tooling. |
| Contract registry tooling | Artifact fingerprinting and bytecode inspection workflows. |

---

## CLI Usage

```console
$ kaspascript compile escrow.ks
escrow.artifact.json
```

```console
$ kaspascript inspect escrow.ks
IR contracts: 1
contract Escrow
  spend release: 10 instructions
  spend refund: 9 instructions
```

```console
$ kaspascript kernel package escrow.ks --compute-grams 1000 --tx-bytes 400
escrow.kernel.json
```

```console
$ kaspascript verify escrow.artifact.json
backend: kaspa-txscript
target: verified-tn12
compiler: 0.1.0
bytecode_bytes: 75
finality_depth: Some(10)
kip_requirements: [10]
```

---

## Contract Patterns

| Pattern | Description | Target status |
| --- | --- | --- |
| Escrow | 2-of-3 release path with timeout refund and output value checks. | Verified TN12 |
| Timelock | Signature spend gated by `OP_CHECKLOCKTIMEVERIFY`. | Verified TN12 |
| Multisig | Static threshold signatures lowered to `OP_CHECKMULTISIG`. | Verified TN12 |
| Atomic swap | Hash preimage-style claim path plus refund timeout. | Verified TN12 |
| Covenant vault | Finality-aware vault pattern using verified txscript constraints today; covenant lineage remains future-gated. | Partial / gated |
| DAGSafe channel | Hash-committed cooperative close, timeout refund, and mediated close using verified script primitives. | Verified TN12 |
| DAGSafeVault kernel blueprint | UTXO covenant state-machine package with wallet previews, indexer schema, TN10 readiness report, and mainnet activation guard. | Kernel / TN10-gated |

Examples live in `tests/contracts`; committed outputs live in `tests/golden`.

---

## Determinism & Verification

KaspaScript keeps bytecode generation measurable.

| Check | Coverage |
| --- | --- |
| Determinism | Escrow compiles 1000 times with identical bytecode and source hash. |
| Golden snapshots | Each example checks artifact JSON, expected hex, and expected ASM. |
| Negative tests | Wrong signature type, invalid input index, bad finality depth, and unsupported covenant features fail. |
| Fuzz smoke | Random lexer/parser input must not panic. |
| Clippy | Workspace is clean under `-D warnings`. |

For the current Toccata/DAGKnight preparation notes, see
[`docs/KASPA_UPGRADE_PREP.md`](docs/KASPA_UPGRADE_PREP.md). That brief records
the latest upstream KIP and Rusty Kaspa checkpoints without unlocking
unsupported bytecode paths prematurely.
For the new framework layer, see
[`docs/KASPA_PROGRAMMABILITY_KERNEL.md`](docs/KASPA_PROGRAMMABILITY_KERNEL.md).
For the kernel package JSON shape, see
[`docs/KERNEL_PACKAGE_SCHEMA.md`](docs/KERNEL_PACKAGE_SCHEMA.md).
For the completion roadmap, see
[`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md).
The crate compatibility spike is in
[`docs/TOCCATA_CRATE_COMPATIBILITY.md`](docs/TOCCATA_CRATE_COMPATIBILITY.md).

---

## Repository Layout

```text
kaspascript/
├── compiler/
│   ├── lexer/       tokenization with line/column/byte spans
│   ├── parser/      AST construction and Pratt expressions
│   ├── semantic/    scope, type, builtin, and finality checks
│   ├── ir/          opcode-agnostic contract IR
│   ├── codegen/     target gates, txscript backend, artifacts
│   └── protocol/    target manifests and feature gates
├── kernel/          Kaspa-native app kernel: blueprints, wallet preview, indexer schema
├── sdk/             Rust compile API and preview transaction model
├── cli/             kaspascript command-line interface
├── tests/
│   ├── contracts/   verified example contracts
│   └── golden/      artifact JSON, hex, and ASM snapshots
├── docs/            source-grounded protocol audit
└── contracts/       future-gated design fixtures
```

---

## Performance & Quality Gates

```console
$ cargo test --workspace
# unit, integration, fuzz-smoke, determinism, and golden tests

$ cargo clippy --workspace --all-targets -- -D warnings
# warning-clean workspace

$ cargo test --features tn12-integration -- --ignored
# live TN12 RPC/wallet preflight and gated proof files

$ cargo bench -p kaspascript-codegen --bench escrow
# escrow full-pipeline benchmark
```

See `docs/TESTNET.md` for the exact TN12 setup, required environment
variables, and proof-file format.

Current benchmark target: full escrow compile pipeline under 1 ms on a typical
developer machine.

---

## Protocol Honesty

KaspaScript does not claim live mainnet smart-contract support.

Verified protocol evidence currently covers base Kaspa txscript behavior and
KIP-10 transaction introspection from pinned Kaspa sources. Covenant IDs, ZK
verification opcodes, and script-visible sequencing remain gated until primary
Kaspa sources define them.

Read the audit: [`docs/kaspa-source-audit.md`](docs/kaspa-source-audit.md).

Support Dev <3 : kaspa:qpv7fcvdlz6th4hqjtm9qkkms2dw0raem963x3hm8glu3kjgj7922vy69hv85
