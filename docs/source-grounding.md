# Source-Grounded Verification Pass

This repository does not currently include upstream Kaspa consensus source files
or KIP documents. The compiler therefore treats local KaspaScript language and
SDK behavior as locally verified, treats protocol/opcode/KIP behavior as gated,
and refuses behavior with no local source-backed lowering.

Status meanings:

- `VERIFIED`: defined by a local KaspaScript source file and covered by tests.
- `GATED`: recognized locally, but dependent on a future local Kaspa consensus
  source before it can be treated as protocol-verified.
- `UNSUPPORTED`: present in an enum or syntax surface but not safe to emit.

## Backend Opcodes

| Item | Status | Local citation |
| --- | --- | --- |
| `OP_PUSH_INT` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_PUSH_BOOL` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_PUSH_BYTES` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_DROP` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_DUP` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_EQUAL` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_VERIFY` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_CHECKSIG` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_CHECKMULTISIG` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_CHECKLOCKTIMEVERIFY` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_ADD`, `OP_SUB`, `OP_MUL` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_DIV`, `OP_MOD` | UNSUPPORTED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_GREATERTHAN`, `OP_GREATERTHANOREQUAL` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_NOT`, `OP_AND`, `OP_OR` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_SHA256`, `OP_BLAKE2B` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_HASH160` | UNSUPPORTED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_INPUTVALUE`, `OP_INPUTSCRIPT` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_OUTPUTVALUE`, `OP_OUTPUTSCRIPT` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_INPUTCOUNT`, `OP_OUTPUTCOUNT` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_COVENANTID`, `OP_COVENANTID_DEPTH` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_ZK_GROTH16_VERIFY` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_ZK_RISCZERO_VERIFY` | UNSUPPORTED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_SEQUENCING_COMMITMENT` | GATED | `compiler/codegen/src/backends/toccata.rs` |
| `OP_CHECK_HASH_PREIMAGE` | UNSUPPORTED | `compiler/codegen/src/backends/toccata.rs` |

## Builtins

| Builtin | Status | Local citation |
| --- | --- | --- |
| `finality_depth` | VERIFIED | `compiler/parser/src/parser.rs` |
| `multisig` | VERIFIED | `compiler/semantic/src/checker.rs` |
| `input`, `output` | GATED | `compiler/semantic/src/checker.rs` |
| `block` | GATED | `compiler/semantic/src/checker.rs` |
| `covenant_id` | GATED | `compiler/semantic/src/checker.rs` |
| `covenant` | UNSUPPORTED | `compiler/semantic/src/checker.rs` |
| `sequencing` | GATED | `compiler/semantic/src/checker.rs` |
| `zk_verify` | GATED | `compiler/semantic/src/checker.rs` |
| `sha256`, `blake2b` | GATED | `compiler/semantic/src/checker.rs` |
| `hash160` | UNSUPPORTED | `compiler/semantic/src/checker.rs` |

## KIP References

| KIP | Status | Local citation |
| --- | --- | --- |
| KIP-16 | GATED | `contracts/production/DAGSafeVault.ks` |
| KIP-17 | GATED | `contracts/production/DAGSafeVault.ks` |
| KIP-20 | GATED | `contracts/production/DAGSafeVault.ks` |
| KIP-21 | GATED | `contracts/production/DAGSafeVault.ks` |

## Transaction Assumptions

| Assumption | Status | Local citation |
| --- | --- | --- |
| Refuse UTXOs below `finality_depth` confirmations | VERIFIED | `sdk/src/lib.rs` |
| Inject 10bps treasury fee at transaction-build time | VERIFIED | `sdk/src/lib.rs` |

## Artifact Fields

| Field | Status | Local citation |
| --- | --- | --- |
| `bytecode` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `source_hash` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `compiler_version` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `backend` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `finality_depth` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `kip_requirements` | VERIFIED | `compiler/codegen/src/lib.rs` |
| `warnings` | VERIFIED | `compiler/codegen/src/lib.rs` |

The executable registry is `compiler/codegen/src/grounding.rs`. Tests assert
that all IR instructions have source-grounding records, unsupported instructions
fail compilation, and gated behavior emits warnings into `CompiledArtifact`.
