//! Source-grounding registry for protocol-sensitive compiler behavior.

use kaspascript_ir::{InstructionKind, Value};
use serde::{Deserialize, Serialize};

/// Source-grounded verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Verified,
    Gated,
    Unsupported,
}

/// Source-grounding category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundingCategory {
    BackendOpcode,
    Builtin,
    KipReference,
    TransactionAssumption,
    ArtifactField,
    TargetGate,
}

/// A local-source citation for a compiler assumption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCitation {
    pub path: String,
    pub detail: String,
}

/// Source-grounding record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundingRecord {
    pub id: &'static str,
    pub category: GroundingCategory,
    pub status: VerificationStatus,
    pub citation: SourceCitation,
    pub note: &'static str,
}

/// Warning emitted into compiled artifacts for gated behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroundingWarning {
    pub id: String,
    pub category: GroundingCategory,
    pub citation: SourceCitation,
    pub message: String,
}

impl GroundingRecord {
    /// Converts a gated record into an artifact warning.
    pub fn warning(&self) -> GroundingWarning {
        GroundingWarning {
            id: self.id.to_owned(),
            category: self.category,
            citation: self.citation.clone(),
            message: self.note.to_owned(),
        }
    }
}

/// Returns all local source-grounding records.
pub fn all_records() -> Vec<GroundingRecord> {
    let mut records = Vec::new();
    records.extend(backend_opcode_records());
    records.extend(builtin_records());
    records.extend(kip_records());
    records.extend(transaction_assumption_records());
    records.extend(artifact_field_records());
    records.extend(target_gate_records());
    records
}

/// Returns records for backend opcode assumptions.
pub fn backend_opcode_records() -> Vec<GroundingRecord> {
    vec![
        verified_opcode(
            "push-int",
            "canonical script-number pushes via OP_0, OP_1..OP_16, OP_DATA_N",
        ),
        verified_opcode("push-bool", "boolean pushes via OP_0 and OP_1"),
        verified_opcode("push-bytes", "canonical pushdata OP_DATA_N/OP_PUSHDATA_N"),
        verified_opcode("drop", "OpDrop (0x75)"),
        verified_opcode("dup", "OpDup (0x76)"),
        verified_opcode("equal", "OpEqual (0x87)"),
        verified_opcode("verify", "OpVerify (0x69)"),
        verified_opcode("checksig", "OpCheckSig (0xac)"),
        verified_opcode("checkmultisig", "OpCheckMultiSig (0xae)"),
        verified_opcode("checklocktimeverify", "OpCheckLockTimeVerify (0xb0)"),
        verified_opcode("add", "OpAdd (0x93)"),
        verified_opcode("sub", "OpSub (0x94)"),
        unsupported_opcode(
            "mul",
            "OpMul exists at 0x95 but rusty-kaspa marks it disabled",
        ),
        unsupported_opcode(
            "div",
            "OpDiv exists at 0x96 but rusty-kaspa marks it disabled",
        ),
        unsupported_opcode(
            "mod",
            "OpMod exists at 0x97 but rusty-kaspa marks it disabled",
        ),
        verified_opcode("lessthan", "OpLessThan (0x9f)"),
        verified_opcode("lessthanorequal", "OpLessThanOrEqual (0xa1)"),
        verified_opcode("greaterthan", "OpGreaterThan (0xa0)"),
        verified_opcode("greaterthanorequal", "OpGreaterThanOrEqual (0xa2)"),
        verified_opcode("not", "OpNot (0x91)"),
        verified_opcode("and", "OpBoolAnd (0x9a)"),
        verified_opcode("or", "OpBoolOr (0x9b)"),
        verified_opcode("notequal", "OpNumNotEqual (0x9e)"),
        verified_opcode("sha256", "OpSHA256 (0xa8)"),
        unsupported_opcode(
            "hash160",
            "no Hash160 opcode is present; 0xa9 is OpCheckMultiSigECDSA",
        ),
        verified_opcode("blake2b", "OpBlake2b (0xaa)"),
        verified_opcode("inputvalue", "OpTxInputAmount (0xbe), KIP-10"),
        verified_opcode("inputscript", "OpTxInputSpk (0xbf), KIP-10"),
        verified_opcode("outputvalue", "OpTxOutputAmount (0xc2), KIP-10"),
        verified_opcode("outputscript", "OpTxOutputSpk (0xc3), KIP-10"),
        verified_opcode("inputcount", "OpTxInputCount (0xb3), KIP-10"),
        verified_opcode("outputcount", "OpTxOutputCount (0xb4), KIP-10"),
        unsupported_opcode(
            "covenantid",
            "no covenant ID opcode is present in pinned sources",
        ),
        unsupported_opcode(
            "covenantid-depth",
            "no covenant ID depth opcode is present in pinned sources",
        ),
        unsupported_opcode(
            "zk-groth16-verify",
            "no Groth16 verifier opcode is present in pinned sources",
        ),
        unsupported_opcode(
            "zk-risczero-verify",
            "no RISC Zero verifier opcode is present in pinned sources",
        ),
        unsupported_opcode(
            "sequencing-commitment",
            "KIP-15 defines a header commitment, not a script opcode",
        ),
        unsupported_opcode(
            "check-hash-preimage",
            "no dedicated hash-preimage opcode is present in pinned sources",
        ),
    ]
}

/// Returns records for KaspaScript builtins.
pub fn builtin_records() -> Vec<GroundingRecord> {
    vec![
        verified_builtin(
            "finality_depth",
            "compiler/parser/src/parser.rs",
            "finality_depth is extracted from params into Contract.finality_depth",
        ),
        verified_builtin(
            "multisig",
            "compiler/semantic/src/checker.rs",
            "multisig arity and static threshold checks are implemented locally; backend emits OpCheckMultiSig",
        ),
        verified_builtin(
            "input",
            "docs/kaspa-source-audit.md",
            "input(n).value/script lower to KIP-10 OpTxInputAmount/OpTxInputSpk",
        ),
        verified_builtin(
            "output",
            "docs/kaspa-source-audit.md",
            "output(n).value/script lower to KIP-10 OpTxOutputAmount/OpTxOutputSpk",
        ),
        gated_builtin(
            "block",
            "block.height/time syntax is recognized; parameterized lock values are still emitted as script-template data",
        ),
        unsupported_builtin("covenant_id", "no pinned Kaspa source defines covenant ID opcodes"),
        unsupported_builtin("covenant", "no pinned Kaspa source defines covenant.with_keys lowering"),
        unsupported_builtin(
            "sequencing",
            "KIP-15 sequencing commitment is a header/archival-node commitment, not a txscript opcode",
        ),
        unsupported_builtin(
            "zk_verify",
            "no pinned Kaspa source defines a txscript ZK verifier opcode",
        ),
        verified_builtin("sha256", "docs/kaspa-source-audit.md", "OpSHA256 is present at 0xa8"),
        verified_builtin("blake2b", "docs/kaspa-source-audit.md", "OpBlake2b is present at 0xaa"),
        unsupported_builtin("hash160", "no Hash160 opcode is present in pinned txscript sources"),
    ]
}

/// Returns records for KIP references.
pub fn kip_records() -> Vec<GroundingRecord> {
    vec![
        verified_kip(
            10,
            "docs/kaspa-source-audit.md",
            "KIP-10 is present in kaspanet/kips and marked Active; rusty-kaspa has matching txscript opcodes",
        ),
        verified_kip(
            15,
            "docs/kaspa-source-audit.md",
            "KIP-15 is present in kaspanet/kips and marked Active as a block-header commitment",
        ),
        gated_kip(
            16,
            "docs/kaspa-source-audit.md",
            "no KIP-16 file or txscript ZK opcode is present in the pinned source set",
        ),
        gated_kip(
            17,
            "docs/kaspa-source-audit.md",
            "no KIP-17 file is present; previous introspection claims map to KIP-10",
        ),
        gated_kip(
            20,
            "docs/kaspa-source-audit.md",
            "no KIP-20 file or covenant ID opcode is present in the pinned source set",
        ),
        gated_kip(
            21,
            "docs/kaspa-source-audit.md",
            "no KIP-21 file is present; sequencing source is KIP-15 and is not a txscript opcode",
        ),
    ]
}

/// Returns records for SDK transaction assumptions.
pub fn transaction_assumption_records() -> Vec<GroundingRecord> {
    vec![
        verified_record(
            "sdk-finality-depth-enforcement",
            GroundingCategory::TransactionAssumption,
            "sdk/src/lib.rs",
            "build_spend_tx rejects UTXOs below artifact.finality_depth confirmations",
            "local SDK behavior is tested; no Kaspa RPC submission source is present",
        ),
        verified_record(
            "sdk-no-hidden-fee",
            GroundingCategory::TransactionAssumption,
            "sdk/src/lib.rs",
            "build_spend_tx routes the full spend value to the contract output and creates no treasury output",
            "hidden treasury fee injection has been removed",
        ),
        gated_record(
            "sdk-transaction-builder-preview",
            GroundingCategory::TransactionAssumption,
            "sdk/src/lib.rs",
            "Transaction is a deterministic SDK model, not a rusty-kaspa Transaction",
            "real Kaspa transaction construction/submission remains preview-gated",
        ),
    ]
}

/// Returns records for compiled artifact fields.
pub fn artifact_field_records() -> Vec<GroundingRecord> {
    [
        "bytecode",
        "source_hash",
        "compiler_version",
        "backend",
        "target",
        "finality_depth",
        "kip_requirements",
        "warnings",
    ]
    .into_iter()
    .map(|field| {
        verified_record(
            field,
            GroundingCategory::ArtifactField,
            "compiler/codegen/src/lib.rs",
            "CompiledArtifact field is defined in the local codegen crate",
            "artifact field is locally defined and covered by artifact metadata tests",
        )
    })
    .collect()
}

/// Returns records for target gate behavior.
pub fn target_gate_records() -> Vec<GroundingRecord> {
    vec![
        verified_record(
            "target-verified-tn12",
            GroundingCategory::TargetGate,
            "compiler/codegen/src/lib.rs",
            "Target::VerifiedTn12 rejects gated and unsupported records",
            "verified TN12 emits only behavior backed by pinned sources",
        ),
        verified_record(
            "target-toccata-preview",
            GroundingCategory::TargetGate,
            "compiler/codegen/src/lib.rs",
            "Target::ToccataPreview warns for gated records and rejects unsupported records",
            "preview behavior is explicit in artifact warnings",
        ),
        verified_record(
            "target-future-mainnet",
            GroundingCategory::TargetGate,
            "compiler/codegen/src/lib.rs",
            "Target::FutureMainnet rejects gated and unsupported records",
            "future mainnet emission stays locked until sources are pinned",
        ),
    ]
}

/// Returns the source-grounding record for an instruction.
pub fn record_for_instruction(kind: &InstructionKind) -> GroundingRecord {
    match kind {
        InstructionKind::Push(Value::Integer(_)) => opcode_record("push-int"),
        InstructionKind::Push(Value::Bool(_)) => opcode_record("push-bool"),
        InstructionKind::Push(
            Value::Bytes(_) | Value::String(_) | Value::Symbol(_) | Value::Type(_),
        ) => opcode_record("push-bytes"),
        InstructionKind::Drop => opcode_record("drop"),
        InstructionKind::Dup => opcode_record("dup"),
        InstructionKind::InputValue(_) => opcode_record("inputvalue"),
        InstructionKind::InputScript(_) => opcode_record("inputscript"),
        InstructionKind::OutputValue(_) => opcode_record("outputvalue"),
        InstructionKind::OutputScript(_) => opcode_record("outputscript"),
        InstructionKind::OutputCount => opcode_record("outputcount"),
        InstructionKind::InputCount => opcode_record("inputcount"),
        InstructionKind::CheckSig { .. } => opcode_record("checksig"),
        InstructionKind::CheckMultiSig { .. } => opcode_record("checkmultisig"),
        InstructionKind::CheckLockHeight(_)
        | InstructionKind::CheckLockTime(_)
        | InstructionKind::CheckLockHeightFromStack
        | InstructionKind::CheckLockTimeFromStack => opcode_record("checklocktimeverify"),
        InstructionKind::CovenantDepth => opcode_record("covenantid-depth"),
        InstructionKind::CovenantId => opcode_record("covenantid"),
        InstructionKind::ZkVerifyGroth16 => opcode_record("zk-groth16-verify"),
        InstructionKind::ZkVerifyRiscZero => opcode_record("zk-risczero-verify"),
        InstructionKind::SequencingCommitment => opcode_record("sequencing-commitment"),
        InstructionKind::Sha256 => opcode_record("sha256"),
        InstructionKind::Blake2b => opcode_record("blake2b"),
        InstructionKind::Hash160 => opcode_record("hash160"),
        InstructionKind::CheckHashPreimage => opcode_record("check-hash-preimage"),
        InstructionKind::Verify => opcode_record("verify"),
        InstructionKind::Equal => opcode_record("equal"),
        InstructionKind::NotEqual => opcode_record("notequal"),
        InstructionKind::LessThan => opcode_record("lessthan"),
        InstructionKind::LessThanOrEqual => opcode_record("lessthanorequal"),
        InstructionKind::GreaterThan => opcode_record("greaterthan"),
        InstructionKind::GreaterThanOrEqual => opcode_record("greaterthanorequal"),
        InstructionKind::And => opcode_record("and"),
        InstructionKind::Or => opcode_record("or"),
        InstructionKind::Not => opcode_record("not"),
        InstructionKind::Add => opcode_record("add"),
        InstructionKind::Sub => opcode_record("sub"),
        InstructionKind::Mul => opcode_record("mul"),
        InstructionKind::Div => opcode_record("div"),
        InstructionKind::Mod => opcode_record("mod"),
    }
}

/// Returns the source-grounding record for a KIP.
pub fn record_for_kip(kip: u16) -> Option<GroundingRecord> {
    kip_records()
        .into_iter()
        .find(|record| record.id == format!("kip-{kip}"))
}

fn opcode_record(id: &str) -> GroundingRecord {
    backend_opcode_records()
        .into_iter()
        .find(|record| record.id == id)
        .unwrap_or_else(|| {
            unsupported_record(
                "unknown-opcode",
                GroundingCategory::BackendOpcode,
                "compiler/ir/src/instructions.rs",
                "instruction has no source-grounding record",
                "backend refuses instructions without source grounding",
            )
        })
}

fn verified_opcode(id: &'static str, detail: &'static str) -> GroundingRecord {
    verified_record(
        id,
        GroundingCategory::BackendOpcode,
        "docs/kaspa-source-audit.md",
        detail,
        "opcode byte mapping is verified against pinned Kaspa txscript sources",
    )
}

fn unsupported_opcode(id: &'static str, detail: &'static str) -> GroundingRecord {
    unsupported_record(
        id,
        GroundingCategory::BackendOpcode,
        "docs/kaspa-source-audit.md",
        detail,
        "backend emission is unsupported until a pinned Kaspa source verifies it",
    )
}

fn verified_builtin(id: &'static str, path: &'static str, detail: &'static str) -> GroundingRecord {
    verified_record(
        id,
        GroundingCategory::Builtin,
        path,
        detail,
        "builtin behavior is source-grounded and covered by tests",
    )
}

fn gated_builtin(id: &'static str, note: &'static str) -> GroundingRecord {
    gated_record(
        id,
        GroundingCategory::Builtin,
        "compiler/semantic/src/checker.rs",
        "builtin is recognized by semantic analysis",
        note,
    )
}

fn unsupported_builtin(id: &'static str, note: &'static str) -> GroundingRecord {
    unsupported_record(
        id,
        GroundingCategory::Builtin,
        "compiler/semantic/src/checker.rs",
        "builtin has no verified backend emission",
        note,
    )
}

fn verified_kip(kip: u16, path: &'static str, note: &'static str) -> GroundingRecord {
    let id = kip_id(kip);
    verified_record(
        id,
        GroundingCategory::KipReference,
        path,
        "KIP source is present in the pinned Kaspa KIP repository",
        note,
    )
}

fn gated_kip(kip: u16, path: &'static str, note: &'static str) -> GroundingRecord {
    let id = kip_id(kip);
    gated_record(
        id,
        GroundingCategory::KipReference,
        path,
        "KIP source is absent or does not define txscript emission in pinned sources",
        note,
    )
}

fn kip_id(kip: u16) -> &'static str {
    match kip {
        10 => "kip-10",
        15 => "kip-15",
        16 => "kip-16",
        17 => "kip-17",
        20 => "kip-20",
        21 => "kip-21",
        _ => "kip-unknown",
    }
}

fn verified_record(
    id: &'static str,
    category: GroundingCategory,
    path: &'static str,
    detail: &'static str,
    note: &'static str,
) -> GroundingRecord {
    GroundingRecord {
        id,
        category,
        status: VerificationStatus::Verified,
        citation: SourceCitation {
            path: path.to_owned(),
            detail: detail.to_owned(),
        },
        note,
    }
}

fn gated_record(
    id: &'static str,
    category: GroundingCategory,
    path: &'static str,
    detail: &'static str,
    note: &'static str,
) -> GroundingRecord {
    GroundingRecord {
        id,
        category,
        status: VerificationStatus::Gated,
        citation: SourceCitation {
            path: path.to_owned(),
            detail: detail.to_owned(),
        },
        note,
    }
}

fn unsupported_record(
    id: &'static str,
    category: GroundingCategory,
    path: &'static str,
    detail: &'static str,
    note: &'static str,
) -> GroundingRecord {
    GroundingRecord {
        id,
        category,
        status: VerificationStatus::Unsupported,
        citation: SourceCitation {
            path: path.to_owned(),
            detail: detail.to_owned(),
        },
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_instruction_has_source_grounding() {
        let instructions = vec![
            InstructionKind::Push(Value::Integer(1)),
            InstructionKind::Drop,
            InstructionKind::Dup,
            InstructionKind::InputValue(0),
            InstructionKind::InputScript(0),
            InstructionKind::OutputValue(0),
            InstructionKind::OutputScript(0),
            InstructionKind::OutputCount,
            InstructionKind::InputCount,
            InstructionKind::CheckSig { key_slot: 0 },
            InstructionKind::CheckMultiSig {
                required: 1,
                key_count: 2,
            },
            InstructionKind::CheckLockHeight(1),
            InstructionKind::CheckLockTime(1),
            InstructionKind::CheckLockHeightFromStack,
            InstructionKind::CheckLockTimeFromStack,
            InstructionKind::CovenantDepth,
            InstructionKind::CovenantId,
            InstructionKind::ZkVerifyGroth16,
            InstructionKind::ZkVerifyRiscZero,
            InstructionKind::SequencingCommitment,
            InstructionKind::Sha256,
            InstructionKind::Blake2b,
            InstructionKind::Hash160,
            InstructionKind::CheckHashPreimage,
            InstructionKind::Verify,
            InstructionKind::Equal,
            InstructionKind::NotEqual,
            InstructionKind::LessThan,
            InstructionKind::LessThanOrEqual,
            InstructionKind::GreaterThan,
            InstructionKind::GreaterThanOrEqual,
            InstructionKind::And,
            InstructionKind::Or,
            InstructionKind::Not,
            InstructionKind::Add,
            InstructionKind::Sub,
            InstructionKind::Mul,
            InstructionKind::Div,
            InstructionKind::Mod,
        ];

        for instruction in instructions {
            let record = record_for_instruction(&instruction);
            assert_ne!(record.id, "unknown-opcode");
            assert!(!record.citation.path.is_empty());
        }
    }

    #[test]
    fn unsupported_instruction_status_is_explicit() {
        let hash160 = record_for_instruction(&InstructionKind::Hash160);
        let covenant = record_for_instruction(&InstructionKind::CovenantId);
        let mul = record_for_instruction(&InstructionKind::Mul);
        assert_eq!(hash160.status, VerificationStatus::Unsupported);
        assert_eq!(covenant.status, VerificationStatus::Unsupported);
        assert_eq!(mul.status, VerificationStatus::Unsupported);
        assert_eq!(hash160.citation.path, "docs/kaspa-source-audit.md");
    }

    #[test]
    fn verified_kip10_and_gated_future_kips_are_separated() {
        assert_eq!(
            record_for_kip(10).expect("kip10").status,
            VerificationStatus::Verified
        );
        assert_eq!(
            record_for_kip(20).expect("kip20").status,
            VerificationStatus::Gated
        );
    }

    #[test]
    fn all_categories_are_represented() {
        let records = all_records();
        for category in [
            GroundingCategory::BackendOpcode,
            GroundingCategory::Builtin,
            GroundingCategory::KipReference,
            GroundingCategory::TransactionAssumption,
            GroundingCategory::ArtifactField,
            GroundingCategory::TargetGate,
        ] {
            assert!(records.iter().any(|record| record.category == category));
        }
    }

    #[test]
    fn builtin_registry_marks_supported_gated_and_unsupported_surfaces() {
        let records = builtin_records();
        let multisig = records
            .iter()
            .find(|record| record.id == "multisig")
            .expect("multisig record");
        let block = records
            .iter()
            .find(|record| record.id == "block")
            .expect("block record");
        let covenant = records
            .iter()
            .find(|record| record.id == "covenant")
            .expect("covenant record");

        assert_eq!(multisig.status, VerificationStatus::Verified);
        assert_eq!(block.status, VerificationStatus::Gated);
        assert_eq!(covenant.status, VerificationStatus::Unsupported);
    }

    #[test]
    fn warning_preserves_exact_local_citation() {
        let warning = record_for_kip(16).expect("kip16").warning();
        assert_eq!(warning.citation.path, "docs/kaspa-source-audit.md");
        assert_eq!(warning.category, GroundingCategory::KipReference);
    }
}
