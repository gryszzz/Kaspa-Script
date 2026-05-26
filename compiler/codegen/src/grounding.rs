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
    records
}

/// Returns records for backend opcode assumptions.
pub fn backend_opcode_records() -> Vec<GroundingRecord> {
    vec![
        gated_opcode("push-int", "OP_PUSH_INT"),
        gated_opcode("push-bool", "OP_PUSH_BOOL"),
        gated_opcode("push-bytes", "OP_PUSH_BYTES"),
        gated_opcode("drop", "OP_DROP"),
        gated_opcode("dup", "OP_DUP"),
        gated_opcode("equal", "OP_EQUAL"),
        gated_opcode("verify", "OP_VERIFY"),
        gated_opcode("checksig", "OP_CHECKSIG"),
        gated_opcode("checkmultisig", "OP_CHECKMULTISIG"),
        gated_opcode("checklocktimeverify", "OP_CHECKLOCKTIMEVERIFY"),
        gated_opcode("add", "OP_ADD"),
        gated_opcode("sub", "OP_SUB"),
        gated_opcode("mul", "OP_MUL"),
        unsupported_opcode("div", "OP_DIV"),
        unsupported_opcode("mod", "OP_MOD"),
        gated_opcode("greaterthan", "OP_GREATERTHAN"),
        gated_opcode("greaterthanorequal", "OP_GREATERTHANOREQUAL"),
        gated_opcode("not", "OP_NOT"),
        gated_opcode("and", "OP_AND"),
        gated_opcode("or", "OP_OR"),
        gated_opcode("sha256", "OP_SHA256"),
        unsupported_opcode("hash160", "OP_HASH160"),
        gated_opcode("blake2b", "OP_BLAKE2B"),
        gated_opcode("inputvalue", "OP_INPUTVALUE"),
        gated_opcode("inputscript", "OP_INPUTSCRIPT"),
        gated_opcode("outputvalue", "OP_OUTPUTVALUE"),
        gated_opcode("outputscript", "OP_OUTPUTSCRIPT"),
        gated_opcode("inputcount", "OP_INPUTCOUNT"),
        gated_opcode("outputcount", "OP_OUTPUTCOUNT"),
        gated_opcode("covenantid", "OP_COVENANTID"),
        gated_opcode("covenantid-depth", "OP_COVENANTID_DEPTH"),
        gated_opcode("zk-groth16-verify", "OP_ZK_GROTH16_VERIFY"),
        unsupported_opcode("zk-risczero-verify", "OP_ZK_RISCZERO_VERIFY"),
        gated_opcode("sequencing-commitment", "OP_SEQUENCING_COMMITMENT"),
        unsupported_opcode("check-hash-preimage", "OP_CHECK_HASH_PREIMAGE"),
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
            "multisig arity and static threshold checks are implemented locally",
        ),
        gated_builtin(
            "input",
            "KIP-17 transaction introspection is not backed by a local Kaspa opcode source",
        ),
        gated_builtin(
            "output",
            "KIP-17 transaction introspection is not backed by a local Kaspa opcode source",
        ),
        gated_builtin(
            "block",
            "lock-height/time lowering is not backed by a local Kaspa opcode source",
        ),
        gated_builtin(
            "covenant_id",
            "KIP-20 covenant ID behavior is not backed by a local Kaspa source",
        ),
        unsupported_builtin(
            "covenant",
            "no local source defines covenant.with_keys lowering",
        ),
        gated_builtin(
            "sequencing",
            "KIP-21 sequencing behavior is not backed by a local Kaspa source",
        ),
        gated_builtin(
            "zk_verify",
            "KIP-16 verifier behavior is not backed by a local Kaspa source",
        ),
        gated_builtin(
            "sha256",
            "hash opcode byte mapping is not backed by a local Kaspa source",
        ),
        gated_builtin(
            "blake2b",
            "hash opcode byte mapping is not backed by a local Kaspa source",
        ),
        unsupported_builtin(
            "hash160",
            "no provided Kaspa source verifies Hash160 backend support",
        ),
    ]
}

/// Returns records for KIP references.
pub fn kip_records() -> Vec<GroundingRecord> {
    vec![
        gated_kip(16, "contracts/production/DAGSafeVault.ks", "ZK verifier dependency is mentioned locally but no Kaspa KIP source file is present"),
        gated_kip(17, "contracts/production/DAGSafeVault.ks", "transaction introspection dependency is mentioned locally but no Kaspa KIP source file is present"),
        gated_kip(20, "contracts/production/DAGSafeVault.ks", "covenant ID dependency is mentioned locally but no Kaspa KIP source file is present"),
        gated_kip(21, "contracts/production/DAGSafeVault.ks", "sequencing dependency is mentioned locally but no Kaspa KIP source file is present"),
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
            "sdk-10bps-fee-injection",
            GroundingCategory::TransactionAssumption,
            "sdk/src/lib.rs",
            "build_spend_tx computes total_value / 1_000 and routes it to kaspascript-treasury",
            "local SDK behavior is tested; treasury policy is not a Kaspa consensus rule",
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

/// Returns the source-grounding record for an instruction.
pub fn record_for_instruction(kind: &InstructionKind) -> GroundingRecord {
    match kind {
        InstructionKind::Push(Value::Integer(_)) => opcode_record("push-int"),
        InstructionKind::Push(Value::Bool(_)) => opcode_record("push-bool"),
        InstructionKind::Push(Value::String(_) | Value::Symbol(_) | Value::Type(_)) => {
            opcode_record("push-bytes")
        }
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
        InstructionKind::CheckLockHeight(_) | InstructionKind::CheckLockTime(_) => {
            opcode_record("checklocktimeverify")
        }
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
        InstructionKind::NotEqual | InstructionKind::Not => opcode_record("not"),
        InstructionKind::GreaterThan => opcode_record("greaterthan"),
        InstructionKind::GreaterThanOrEqual => opcode_record("greaterthanorequal"),
        InstructionKind::And => opcode_record("and"),
        InstructionKind::Or => opcode_record("or"),
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

fn gated_opcode(id: &'static str, constant: &'static str) -> GroundingRecord {
    gated_record(
        id,
        GroundingCategory::BackendOpcode,
        "compiler/codegen/src/backends/toccata.rs",
        constant,
        "opcode byte mapping is gated because no local Kaspa consensus opcode table is present",
    )
}

fn unsupported_opcode(id: &'static str, constant: &'static str) -> GroundingRecord {
    unsupported_record(
        id,
        GroundingCategory::BackendOpcode,
        "compiler/codegen/src/backends/toccata.rs",
        constant,
        "opcode byte mapping is unsupported until a local Kaspa source verifies it",
    )
}

fn verified_builtin(id: &'static str, path: &'static str, detail: &'static str) -> GroundingRecord {
    verified_record(
        id,
        GroundingCategory::Builtin,
        path,
        detail,
        "builtin behavior is defined by local compiler source and covered by tests",
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
        "builtin is not fully lowered by the compiler",
        note,
    )
}

fn gated_kip(kip: u16, path: &'static str, note: &'static str) -> GroundingRecord {
    let id = match kip {
        16 => "kip-16",
        17 => "kip-17",
        20 => "kip-20",
        21 => "kip-21",
        _ => "kip-unknown",
    };
    gated_record(
        id,
        GroundingCategory::KipReference,
        path,
        "local contract/doc references this KIP",
        note,
    )
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
        let record = record_for_instruction(&InstructionKind::Hash160);
        assert_eq!(record.status, VerificationStatus::Unsupported);
        assert_eq!(
            record.citation.path,
            "compiler/codegen/src/backends/toccata.rs"
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
        let input = records
            .iter()
            .find(|record| record.id == "input")
            .expect("input record");
        let covenant = records
            .iter()
            .find(|record| record.id == "covenant")
            .expect("covenant record");

        assert_eq!(multisig.status, VerificationStatus::Verified);
        assert_eq!(input.status, VerificationStatus::Gated);
        assert_eq!(covenant.status, VerificationStatus::Unsupported);
    }

    #[test]
    fn warning_preserves_exact_local_citation() {
        let warning = record_for_instruction(&InstructionKind::InputValue(0)).warning();
        assert_eq!(
            warning.citation.path,
            "compiler/codegen/src/backends/toccata.rs"
        );
        assert_eq!(warning.category, GroundingCategory::BackendOpcode);
    }
}
