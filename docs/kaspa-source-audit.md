# Kaspa Source Reality Audit

Audit date: 2026-05-26.

Pinned upstream sources:

- `kaspanet/rusty-kaspa` commit `a07d8b38d45f38a02a1f35f601e874358f6c7846`
- `kaspanet/kips` commit `2a77c954b2241bce7954ba5fecad0ac7694ce195`

Only the files listed below are treated as protocol evidence. Project roadmap
text, model memory, and unsourced prompts are not protocol evidence.

## Verified Kaspa Sources

| Evidence | Upstream source | Finding |
| --- | --- | --- |
| Base txscript opcodes | `crypto/txscript/src/opcodes/mod.rs` | Defines canonical push opcodes, stack opcodes, arithmetic/comparison opcodes, hash opcodes, signature opcodes, and locktime opcodes. |
| Canonical pushes | `crypto/txscript/src/script_builder.rs` | Defines canonical data and integer push behavior using `Op0`, `Op1..Op16`, `OpDataN`, and `OpPushDataN`. |
| Transaction introspection | `crypto/txscript/src/opcodes/mod.rs` and `kip-0010.md` | Verifies KIP-10 opcodes `OpTxInputCount` `0xb3`, `OpTxOutputCount` `0xb4`, `OpTxInputAmount` `0xbe`, `OpTxInputSpk` `0xbf`, `OpTxOutputAmount` `0xc2`, and `OpTxOutputSpk` `0xc3`. |
| Sequencing commitment | `kip-0015.md` | Verifies KIP-15 as a block-header / accepted-transaction-ordering commitment. It is not a txscript opcode. |

## Backend Opcode Decisions

| KaspaScript item | Status | Source |
| --- | --- | --- |
| Canonical integer, bool, and byte pushes | VERIFIED | `crypto/txscript/src/script_builder.rs`; implemented in `compiler/codegen/src/backends/toccata.rs`. |
| `Drop`, `Dup`, `Verify`, `Equal` | VERIFIED | `crypto/txscript/src/opcodes/mod.rs`; implemented in `compiler/codegen/src/backends/toccata.rs`. |
| `CheckSig`, `CheckMultiSig` | VERIFIED | `crypto/txscript/src/opcodes/mod.rs`; implemented in `compiler/codegen/src/backends/toccata.rs`. |
| `CheckLockHeight`, `CheckLockTime` | VERIFIED | `OpCheckLockTimeVerify` `0xb0` in `crypto/txscript/src/opcodes/mod.rs`; implemented in `compiler/codegen/src/backends/toccata.rs`. |
| `Add`, `Sub`, comparisons, boolean and/or/not | VERIFIED | `crypto/txscript/src/opcodes/mod.rs`; implemented in `compiler/codegen/src/backends/toccata.rs`. |
| `Sha256`, `Blake2b` | VERIFIED | `OpSHA256` `0xa8` and `OpBlake2b` `0xaa` in `crypto/txscript/src/opcodes/mod.rs`. |
| `InputValue`, `InputScript`, `OutputValue`, `OutputScript`, `InputCount`, `OutputCount` | VERIFIED | KIP-10 plus matching `crypto/txscript/src/opcodes/mod.rs` implementations. |
| `Mul`, `Div`, `Mod` | UNSUPPORTED | Present in `crypto/txscript/src/opcodes/mod.rs` but explicitly disabled by the engine. Compilation fails. |
| `Hash160` | UNSUPPORTED | No Hash160 opcode exists in the pinned txscript source; byte `0xa9` is `OpCheckMultiSigECDSA`. Compilation fails. |
| `CovenantId`, `CovenantDepth` | UNSUPPORTED | No covenant ID opcode exists in the pinned source set. Compilation fails. |
| `ZkVerifyGroth16`, `ZkVerifyRiscZero` | UNSUPPORTED | No txscript ZK verifier opcode exists in the pinned source set. Compilation fails. |
| `SequencingCommitment` | UNSUPPORTED | KIP-15 is a header commitment, not a txscript opcode. Compilation fails. |
| `CheckHashPreimage` | UNSUPPORTED | No dedicated hash-preimage opcode exists in the pinned source set. Compilation fails. |

## Builtin Decisions

| Builtin | Status | Source |
| --- | --- | --- |
| `finality_depth` | VERIFIED | Parsed into `Contract.finality_depth` in `compiler/parser/src/parser.rs`; enforced in `compiler/semantic/src/checker.rs` and `sdk/src/lib.rs`. |
| `multisig` | VERIFIED | Checked in `compiler/semantic/src/checker.rs`; lowered in `compiler/ir/src/gen.rs`; emitted as `OpCheckMultiSig`. |
| `input`, `output` | VERIFIED | KIP-10 and `crypto/txscript/src/opcodes/mod.rs`; lowered in `compiler/ir/src/gen.rs`. |
| `block.height`, `block.time` | GATED | `OpCheckLockTimeVerify` is verified, but parameterized lock values are script-template placeholders until transaction instantiation is implemented. |
| `sha256`, `blake2b` | VERIFIED | `crypto/txscript/src/opcodes/mod.rs`. |
| `hash160` | UNSUPPORTED | No pinned txscript opcode. |
| `covenant`, `covenant_id` | UNSUPPORTED | No pinned txscript opcode or KIP source. |
| `zk_verify` | UNSUPPORTED | No pinned txscript verifier opcode or KIP source. |
| `sequencing` | UNSUPPORTED | KIP-15 is not a txscript opcode. |

## KIP And Toccata Claims

| Claim | Status | Source |
| --- | --- | --- |
| KIP-10 transaction introspection | VERIFIED | `kip-0010.md`; matching txscript opcodes in `crypto/txscript/src/opcodes/mod.rs`. |
| KIP-15 sequencing commitment | VERIFIED | `kip-0015.md`; header commitment only. |
| KIP-16 ZK verification opcodes | GATED | No `kip-0016.md` and no matching txscript opcodes in pinned sources. |
| KIP-17 covenant/introspection opcodes | GATED | No `kip-0017.md`; verified introspection source is KIP-10. |
| KIP-20 covenant IDs | GATED | No `kip-0020.md` and no matching txscript opcodes in pinned sources. |
| KIP-21 sequencing script access | GATED | No `kip-0021.md`; verified sequencing source is KIP-15 and not script-accessible. |
| “Toccata is live / mainnet smart contracts” | UNSUPPORTED | No `Toccata` term or activation evidence appears in the pinned source set. README must not claim mainnet smart-contract support. |

## Transaction Assumptions

| Assumption | Status | Source |
| --- | --- | --- |
| SDK finality-depth refusal | VERIFIED | Implemented and tested in `sdk/src/lib.rs`. |
| Hidden treasury fee injection | UNSUPPORTED / REMOVED | Removed from `sdk/src/lib.rs`; tests prove no treasury output is injected. |
| Real Kaspa transaction construction | GATED | `sdk/src/lib.rs` exposes a deterministic preview transaction model, not a `rusty-kaspa` transaction. |

## Artifact Fields

| Field | Status | Source |
| --- | --- | --- |
| `bytecode` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; checked by golden artifacts. |
| `source_hash` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; deterministic test covers repeated compilation. |
| `compiler_version` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`. |
| `backend` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; current value is `kaspa-txscript`. |
| `target` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; separates `verified-tn12`, `toccata-preview`, and `future-mainnet`. |
| `finality_depth` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; sourced from parsed contract metadata. |
| `kip_requirements` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; KIP-10 is emitted for transaction introspection. |
| `warnings` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; preview target warnings are tested. |
