# Kaspa Source Reality Audit

Audit date: 2026-06-04.

Compiler baseline sources:

- `kaspanet/rusty-kaspa` commit `a07d8b38d45f38a02a1f35f601e874358f6c7846`
- `kaspanet/kips` commit `2a77c954b2241bce7954ba5fecad0ac7694ce195`

Current upstream learning checkpoint:

- `kaspanet/rusty-kaspa` tag `v2.0.0` commit `90dbf074275d60c1fe74a3491883196f110970c0`
- `kaspanet/rusty-kaspa` tag `v1.3.0-toc.5` commit `04b0d135f8c8023676ea74dcf496c99d5d0bc2a5`
- `kaspanet/rusty-kaspa` tag `tn10-toc3` commit `1015a62359e0d06e0b3b3b7f7d06bc1bd4bf0c1b`
- `kaspanet/kips` `master` commit `1aba3b8321c1d27e00b7d87bd7c74ef879efabdc`

Only the files listed below are treated as protocol evidence. Project roadmap
text, model memory, market articles, and unsourced prompts are not protocol
evidence.

The June 5, 2026 `v2.0.0` Rusty Kaspa release is the mainnet Toccata release.
Its notes schedule activation at DAA score `474,165,565`, roughly
2026-06-30 16:15 UTC. At this audit date, KaspaScript treats `v2.0.0` as
mainnet pre-activation evidence until activation is independently verified.

## Verified Kaspa Sources

| Evidence | Upstream source | Finding |
| --- | --- | --- |
| Base txscript opcodes | `crypto/txscript/src/opcodes/mod.rs` | Defines canonical push opcodes, stack opcodes, arithmetic/comparison opcodes, hash opcodes, signature opcodes, and locktime opcodes. |
| Canonical pushes | `crypto/txscript/src/script_builder.rs` | Defines canonical data and integer push behavior using `Op0`, `Op1..Op16`, `OpDataN`, and `OpPushDataN`. |
| Transaction introspection | `crypto/txscript/src/opcodes/mod.rs` and `kip-0010.md` | Verifies KIP-10 opcodes `OpTxInputCount` `0xb3`, `OpTxOutputCount` `0xb4`, `OpTxInputAmount` `0xbe`, `OpTxInputSpk` `0xbf`, `OpTxOutputAmount` `0xc2`, and `OpTxOutputSpk` `0xc3`. |
| Sequencing commitment | `kip-0015.md` | Verifies KIP-15 as a block-header / accepted-transaction-ordering commitment. It is not a txscript opcode. |
| Toccata mainnet release | `rusty-kaspa` release `v2.0.0` | Verifies a released mainnet upgrade artifact and scheduled activation DAA score. It does not prove activation has already occurred at this audit date. |
| Toccata pre-activation release | `rusty-kaspa` release `v1.3.0-toc.5` | Verifies upstream mainnet sanity testing, the upcoming RPC minimum-standard-fee policy, and one-way node DB upgrade warning. It does not activate Toccata on mainnet. |
| TN10 Toccata hardening | `rusty-kaspa` release `tn10-toc3` | Verifies TN10 activation of final Toccata hardening on May 28, 2026, including Groth16 verifier hardening, ZK pricing behavior, and SMT/seqcommit inactivity shortcut. |
| KIP-16 ZK precompile | `kip-0016.md`; `crypto/txscript/src/opcodes/mod.rs`; `crypto/txscript/src/zk_precompiles/mod.rs` | Defines `OpZkPrecompile` `0xa6` with Groth16 tag `0x20` and RISC0-Succinct tag `0x21`. |
| KIP-17 covenant-era script extensions | `kip-0017.md`; `crypto/txscript/src/opcodes/mod.rs` | Defines expanded transaction introspection, `OpCat`, `OpSubstr`, bitwise ops, `OpMul` / `OpDiv` / `OpMod`, keyed hashes, `OpBlake3`, and signature-from-stack opcodes. |
| KIP-20 covenant IDs | `kip-0020.md`; `crypto/txscript/src/opcodes/mod.rs` | Defines consensus-tracked covenant IDs and script accessors such as `OpInputCovenantId`, `OpOutputCovenantId`, and authorized-output context opcodes. |
| KIP-21 partitioned sequencing | `kip-0021.md`; `crypto/txscript/src/opcodes/mod.rs` | Defines partitioned sequencing commitments and `OpChainblockSeqCommit` `0xd4` for script access to chain-block sequencing commitments. |

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
| `Mul`, `Div`, `Mod` | UNSUPPORTED | The compiler baseline has these disabled. Current Toccata sources enable them behind `covenants_enabled`, but KaspaScript has no activation-safe target/lowering tests yet. Compilation fails. |
| `Hash160` | UNSUPPORTED | No Hash160 opcode exists in the pinned txscript source; byte `0xa9` is `OpCheckMultiSigECDSA`. Compilation fails. |
| `CovenantId`, `CovenantDepth` | UNSUPPORTED | Current upstream defines covenant ID accessors, but KaspaScript IR does not yet distinguish input/output covenant access or emit covenant-bound transaction outputs. Compilation fails. |
| `ZkVerifyGroth16`, `ZkVerifyRiscZero` | UNSUPPORTED | Current upstream defines `OpZkPrecompile`, but KaspaScript has no verified stack ABI lowering, proof payload model, or live proof fixture. Compilation fails. |
| `SequencingCommitment` | UNSUPPORTED | Current upstream defines `OpChainblockSeqCommit`; KaspaScript has no block-hash/depth witness model or activation-safe proof coverage yet. Compilation fails. |
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
| `covenant`, `covenant_id` | UNSUPPORTED | Upstream Toccata sources now define covenant ID primitives; KaspaScript lowering and transaction builder support are not implemented. |
| `zk_verify` | UNSUPPORTED | Upstream Toccata sources now define `OpZkPrecompile`; KaspaScript lowering and proof artifact support are not implemented. |
| `sequencing` | UNSUPPORTED | Upstream Toccata sources now define `OpChainblockSeqCommit`; KaspaScript lowering and witness/depth policy are not implemented. |

## KIP And Toccata Claims

| Claim | Status | Source |
| --- | --- | --- |
| KIP-10 transaction introspection | VERIFIED | `kip-0010.md`; matching txscript opcodes in `crypto/txscript/src/opcodes/mod.rs`. |
| KIP-15 sequencing commitment | VERIFIED | `kip-0015.md`; header commitment only. |
| KIP-16 ZK verification opcodes | GATED | KIP and opcode source exist in current upstream; compiler lowering/proof fixtures are not verified. |
| KIP-17 covenant/introspection opcodes | GATED | KIP and opcode source exist in current upstream; only the older KIP-10 subset is emitted today. |
| KIP-20 covenant IDs | GATED | KIP and opcode source exist in current upstream; transaction-output covenant bindings and covenant ID lowering are not implemented. |
| KIP-21 sequencing script access | GATED | KIP and opcode source exist in current upstream; `OpChainblockSeqCommit` witness policy and tests are not implemented. |
| “Toccata is live / mainnet smart contracts” | UNSUPPORTED | `v2.0.0` schedules mainnet activation for DAA score `474,165,565`; activation has not been independently verified at this audit date. README must not claim active mainnet smart-contract support. |

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
| `target` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; separates `verified-tn12`, `tn10-toccata`, `toccata-preview`, and `future-mainnet`. |
| `finality_depth` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; sourced from parsed contract metadata. |
| `kip_requirements` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; KIP-10 is emitted for transaction introspection. |
| `warnings` | VERIFIED | Defined in `compiler/codegen/src/lib.rs`; preview target warnings are tested. |
