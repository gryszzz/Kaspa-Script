# Source-Grounded Verification Pass

The executable registry is `compiler/codegen/src/grounding.rs`. Its local
evidence base is `docs/kaspa-source-audit.md`, which records the exact pinned
Kaspa sources used for this pass.

Status meanings:

- `VERIFIED`: backed by a pinned Kaspa source or a local KaspaScript source and covered by tests.
- `GATED`: recognized as a future/preview surface, but not allowed for verified TN12 bytecode.
- `UNSUPPORTED`: must fail compilation before bytecode emission.

As of 2026-06-04, upstream Toccata sources exist for KIP-16, KIP-17,
KIP-20, and KIP-21. KaspaScript still treats their contract-facing bytecode as
unsupported until the compiler has exact stack ABI lowering, transaction
builder support, and live testnet proof coverage.

## Backend Opcodes

| Item | Status | Local citation |
| --- | --- | --- |
| Canonical pushes | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_DROP`, `OP_DUP`, `OP_EQUAL`, `OP_VERIFY` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_CHECKSIG`, `OP_CHECKMULTISIG` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_CHECKLOCKTIMEVERIFY` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_ADD`, `OP_SUB` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_MUL`, `OP_DIV`, `OP_MOD` | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| `OP_LESSTHAN`, `OP_GREATERTHAN`, `OP_LESSTHANOREQUAL`, `OP_GREATERTHANOREQUAL` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_NOT`, `OP_BOOLAND`, `OP_BOOLOR`, `OP_NUMNOTEQUAL` | VERIFIED | `docs/kaspa-source-audit.md` |
| `OP_SHA256`, `OP_BLAKE2B` | VERIFIED | `docs/kaspa-source-audit.md` |
| KIP-10 input/output introspection opcodes | VERIFIED | `docs/kaspa-source-audit.md` |
| Covenant ID opcodes | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| ZK verifier opcode | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| Sequencing txscript opcode | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| Dedicated hash-preimage opcode | UNSUPPORTED | `docs/kaspa-source-audit.md` |

## Builtins

| Builtin | Status | Local citation |
| --- | --- | --- |
| `finality_depth` | VERIFIED | `compiler/parser/src/parser.rs`, `compiler/semantic/src/checker.rs`, `sdk/src/lib.rs` |
| `multisig` | VERIFIED | `compiler/semantic/src/checker.rs`, `compiler/ir/src/gen.rs` |
| `input`, `output` | VERIFIED | `docs/kaspa-source-audit.md`, `compiler/ir/src/gen.rs` |
| `block.height`, `block.time` | GATED | `docs/kaspa-source-audit.md` |
| `sha256`, `blake2b` | VERIFIED | `docs/kaspa-source-audit.md` |
| `hash160` | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| `covenant`, `covenant_id` | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| `zk_verify` | UNSUPPORTED | `docs/kaspa-source-audit.md` |
| `sequencing` | UNSUPPORTED | `docs/kaspa-source-audit.md` |

## KIP References

| KIP | Status | Local citation |
| --- | --- | --- |
| KIP-10 | VERIFIED | `docs/kaspa-source-audit.md` |
| KIP-15 | VERIFIED | `docs/kaspa-source-audit.md` |
| KIP-16 | GATED | `docs/kaspa-source-audit.md` |
| KIP-17 | GATED | `docs/kaspa-source-audit.md` |
| KIP-20 | GATED | `docs/kaspa-source-audit.md` |
| KIP-21 | GATED | `docs/kaspa-source-audit.md` |

## Target Gates

| Target | Behavior | Local citation |
| --- | --- | --- |
| `verified-tn12` | Emits only verified records; gated and unsupported records fail. | `compiler/codegen/src/lib.rs`, `compiler/codegen/src/backends/toccata.rs` |
| `tn10-toccata` | Emits verified records and warns for gated records under activated-testnet package posture; unsupported lowering still fails. | `compiler/codegen/src/lib.rs`, `compiler/codegen/src/backends/toccata.rs` |
| `toccata-preview` | Emits verified records, warns for gated records, fails unsupported records. | `compiler/codegen/src/lib.rs`, `compiler/codegen/src/backends/toccata.rs` |
| `future-mainnet` | Fails gated and unsupported records until mainnet sources are pinned. | `compiler/codegen/src/lib.rs`, `compiler/codegen/src/backends/toccata.rs` |

## Transaction Assumptions

| Assumption | Status | Local citation |
| --- | --- | --- |
| Refuse UTXOs below `finality_depth` confirmations | VERIFIED | `sdk/src/lib.rs` |
| No hidden treasury fee injection | VERIFIED | `sdk/src/lib.rs` |
| Real Kaspa transaction construction/submission | GATED | `sdk/src/lib.rs` |

## Artifact Fields

| Field | Status | Local citation |
| --- | --- | --- |
| `bytecode` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `source_hash` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `compiler_version` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `backend` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `target` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `finality_depth` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `kip_requirements` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `warnings` | VERIFIED | `compiler/codegen/src/lib.rs` |

Tests assert that all IR instructions have source-grounding records, verified
contracts compile to committed golden artifact JSON/hex/ASM, unsupported
features fail compilation, preview-gated features warn only under the preview
target, and repeated compilation is deterministic.
