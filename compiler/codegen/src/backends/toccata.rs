use std::collections::HashSet;

use sha2::{Digest, Sha256};

use kaspascript_ir::{Instruction, InstructionKind, IrProgram, Value};
use kaspascript_lexer::Span;

use crate::grounding::{
    record_for_instruction, record_for_kip, GroundingWarning, VerificationStatus,
};
use crate::{
    ArtifactContract, ArtifactParam, ArtifactSpend, CodegenError, CompiledArtifact, Target,
};

const OP_FALSE: u8 = 0x00;
const OP_PUSHDATA1: u8 = 0x4c;
const OP_PUSHDATA2: u8 = 0x4d;
const OP_PUSHDATA4: u8 = 0x4e;
const OP_1: u8 = 0x51;
const OP_DROP: u8 = 0x75;
const OP_DUP: u8 = 0x76;
const OP_EQUAL: u8 = 0x87;
const OP_VERIFY: u8 = 0x69;
const OP_NOT: u8 = 0x91;
const OP_ADD: u8 = 0x93;
const OP_SUB: u8 = 0x94;
const OP_BOOLAND: u8 = 0x9a;
const OP_BOOLOR: u8 = 0x9b;
const OP_NUMNOTEQUAL: u8 = 0x9e;
const OP_LESSTHAN: u8 = 0x9f;
const OP_GREATERTHAN: u8 = 0xa0;
const OP_LESSTHANOREQUAL: u8 = 0xa1;
const OP_GREATERTHANOREQUAL: u8 = 0xa2;
const OP_SHA256: u8 = 0xa8;
const OP_BLAKE2B: u8 = 0xaa;
const OP_CHECKSIG: u8 = 0xac;
const OP_CHECKMULTISIG: u8 = 0xae;
const OP_CHECKLOCKTIMEVERIFY: u8 = 0xb0;
const OP_TXINPUTCOUNT: u8 = 0xb3;
const OP_TXOUTPUTCOUNT: u8 = 0xb4;
const OP_TXINPUTAMOUNT: u8 = 0xbe;
const OP_TXINPUTSPK: u8 = 0xbf;
const OP_TXOUTPUTAMOUNT: u8 = 0xc2;
const OP_TXOUTPUTSPK: u8 = 0xc3;

/// Compiles IR into deterministic Kaspa transaction script bytecode.
pub fn compile_toccata(
    source: &str,
    ir: &IrProgram,
    compiler_version: &str,
    target: Target,
) -> Result<CompiledArtifact, CodegenError> {
    let mut bytecode = Vec::new();
    let mut warnings = Vec::new();
    let mut warning_ids = HashSet::new();

    for kip in &ir.kip_requirements {
        if let Some(record) = record_for_kip(*kip) {
            collect_grounding(&record, target, &mut warnings, &mut warning_ids)?;
        }
    }

    for contract in &ir.contracts {
        for spend in &contract.spends {
            collect_instruction_grounding(
                &spend.instructions,
                target,
                &mut warnings,
                &mut warning_ids,
            )?;
            encode_instruction_sequence_into(&spend.instructions, &mut bytecode)?;
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let source_hash = hasher.finalize().into();

    Ok(CompiledArtifact {
        bytecode,
        source_hash,
        compiler_version: compiler_version.to_owned(),
        backend: "kaspa-txscript".to_owned(),
        target: target.as_str().to_owned(),
        finality_depth: ir
            .contracts
            .iter()
            .filter_map(|contract| contract.finality_depth)
            .max(),
        kip_requirements: ir.kip_requirements.clone(),
        warnings,
        application: ir.application.clone(),
        contracts: ir
            .contracts
            .iter()
            .map(|contract| ArtifactContract {
                name: contract.name.clone(),
                params: contract
                    .params
                    .iter()
                    .map(|param| ArtifactParam {
                        name: param.name.clone(),
                        ty: param.ty,
                    })
                    .collect(),
                finality_depth: contract.finality_depth,
                spends: contract
                    .spends
                    .iter()
                    .map(|spend| ArtifactSpend {
                        name: spend.name.clone(),
                        params: spend
                            .params
                            .iter()
                            .map(|param| ArtifactParam {
                                name: param.name.clone(),
                                ty: param.ty,
                            })
                            .collect(),
                        instructions: spend.instructions.clone(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// Compiles a single instruction sequence into Kaspa script bytes.
pub fn compile_instruction_sequence(instructions: &[Instruction]) -> Result<Vec<u8>, CodegenError> {
    let mut bytecode = Vec::new();
    encode_instruction_sequence_into(instructions, &mut bytecode)?;
    Ok(bytecode)
}

fn collect_grounding(
    record: &crate::grounding::GroundingRecord,
    target: Target,
    warnings: &mut Vec<GroundingWarning>,
    warning_ids: &mut HashSet<String>,
) -> Result<(), CodegenError> {
    match record.status {
        VerificationStatus::Verified => Ok(()),
        VerificationStatus::Gated if target.allows_gated_warnings() => {
            if warning_ids.insert(record.id.to_owned()) {
                warnings.push(record.warning());
            }
            Ok(())
        }
        VerificationStatus::Gated => Err(CodegenError::GatedGrounding {
            id: record.id.to_owned(),
            target: target.as_str().to_owned(),
            citation: record.citation.clone(),
            message: record.note.to_owned(),
        }),
        VerificationStatus::Unsupported => Err(CodegenError::UnsupportedGrounding {
            id: record.id.to_owned(),
            citation: record.citation.clone(),
            message: record.note.to_owned(),
        }),
    }
}

fn collect_instruction_grounding(
    instructions: &[Instruction],
    target: Target,
    warnings: &mut Vec<GroundingWarning>,
    warning_ids: &mut HashSet<String>,
) -> Result<(), CodegenError> {
    for instruction in instructions {
        let record = record_for_instruction(&instruction.kind);
        collect_grounding(&record, target, warnings, warning_ids)?;
    }
    Ok(())
}

fn encode_instruction_sequence_into(
    instructions: &[Instruction],
    out: &mut Vec<u8>,
) -> Result<(), CodegenError> {
    for instruction in instructions {
        encode_instruction(instruction, out)?;
    }
    Ok(())
}

fn encode_instruction(instruction: &Instruction, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    match &instruction.kind {
        InstructionKind::Push(value) => encode_push(value, instruction.span, out)?,
        InstructionKind::Drop => out.push(OP_DROP),
        InstructionKind::Dup => out.push(OP_DUP),
        InstructionKind::InputValue(index) => encode_indexed(OP_TXINPUTAMOUNT, *index, out)?,
        InstructionKind::InputScript(index) => encode_indexed(OP_TXINPUTSPK, *index, out)?,
        InstructionKind::OutputValue(index) => encode_indexed(OP_TXOUTPUTAMOUNT, *index, out)?,
        InstructionKind::OutputScript(index) => encode_indexed(OP_TXOUTPUTSPK, *index, out)?,
        InstructionKind::OutputCount => out.push(OP_TXOUTPUTCOUNT),
        InstructionKind::InputCount => out.push(OP_TXINPUTCOUNT),
        InstructionKind::CheckSig { .. } => out.push(OP_CHECKSIG),
        InstructionKind::CheckMultiSig { key_count, .. } => {
            encode_u64(u64::from(*key_count), instruction.span, out)?;
            out.push(OP_CHECKMULTISIG);
        }
        InstructionKind::CheckLockHeight(height) | InstructionKind::CheckLockTime(height) => {
            encode_u64(*height, instruction.span, out)?;
            out.push(OP_CHECKLOCKTIMEVERIFY);
            out.push(OP_1);
        }
        InstructionKind::CheckLockHeightFromStack | InstructionKind::CheckLockTimeFromStack => {
            out.push(OP_CHECKLOCKTIMEVERIFY);
            out.push(OP_1);
        }
        InstructionKind::CovenantDepth
        | InstructionKind::CovenantId
        | InstructionKind::ZkVerifyGroth16
        | InstructionKind::ZkVerifyRiscZero
        | InstructionKind::SequencingCommitment
        | InstructionKind::Hash160
        | InstructionKind::CheckHashPreimage
        | InstructionKind::Mul
        | InstructionKind::Div
        | InstructionKind::Mod => {
            let record = record_for_instruction(&instruction.kind);
            return Err(CodegenError::UnsupportedGrounding {
                id: record.id.to_owned(),
                citation: record.citation,
                message: record.note.to_owned(),
            });
        }
        InstructionKind::Sha256 => out.push(OP_SHA256),
        InstructionKind::Blake2b => out.push(OP_BLAKE2B),
        InstructionKind::Verify => out.push(OP_VERIFY),
        InstructionKind::Equal => out.push(OP_EQUAL),
        InstructionKind::NotEqual => out.push(OP_NUMNOTEQUAL),
        InstructionKind::GreaterThan => out.push(OP_GREATERTHAN),
        InstructionKind::GreaterThanOrEqual => out.push(OP_GREATERTHANOREQUAL),
        InstructionKind::LessThan => out.push(OP_LESSTHAN),
        InstructionKind::LessThanOrEqual => out.push(OP_LESSTHANOREQUAL),
        InstructionKind::And => out.push(OP_BOOLAND),
        InstructionKind::Or => out.push(OP_BOOLOR),
        InstructionKind::Not => out.push(OP_NOT),
        InstructionKind::Add => out.push(OP_ADD),
        InstructionKind::Sub => out.push(OP_SUB),
    }
    Ok(())
}

fn encode_push(value: &Value, span: Span, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    match value {
        Value::Integer(value) => encode_u64(*value, span, out)?,
        Value::Bool(value) => out.push(if *value { OP_1 } else { OP_FALSE }),
        Value::Bytes(value) => encode_data(value, span, out)?,
        Value::String(value) | Value::Symbol(value) => encode_data(value.as_bytes(), span, out)?,
        Value::Type(ty) => {
            let text = format!("{ty:?}");
            encode_data(text.as_bytes(), span, out)?;
        }
    }
    Ok(())
}

fn encode_indexed(opcode: u8, index: u32, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    encode_u64(u64::from(index), Span::new(0, 0), out)?;
    out.push(opcode);
    Ok(())
}

fn encode_u64(value: u64, span: Span, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    if value == 0 {
        out.push(OP_FALSE);
        return Ok(());
    }
    if value <= 16 {
        out.push(OP_1 + (value as u8) - 1);
        return Ok(());
    }

    let bytes = value.to_le_bytes();
    let trimmed_size = bytes
        .iter()
        .rposition(|byte| *byte != 0)
        .map_or(0, |index| index + 1);
    encode_data(&bytes[..trimmed_size], span, out)
}

fn encode_data(bytes: &[u8], span: Span, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    let len = bytes.len();
    if len <= 75 {
        out.push(len as u8);
    } else if len <= u8::MAX as usize {
        out.push(OP_PUSHDATA1);
        out.push(len as u8);
    } else if len <= u16::MAX as usize {
        out.push(OP_PUSHDATA2);
        out.extend((len as u16).to_le_bytes());
    } else {
        let len = u32::try_from(len).map_err(|_| CodegenError::ValueTooLarge {
            span,
            message: "push value exceeds u32 length".to_owned(),
        })?;
        out.push(OP_PUSHDATA4);
        out.extend(len.to_le_bytes());
    }
    out.extend(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspascript_ir::{IrContract, IrSpend};

    #[test]
    fn emits_kaspa_canonical_pushes() {
        let span = Span::new(0, 1);
        let mut out = Vec::new();

        encode_push(&Value::Integer(0), span, &mut out).expect("zero");
        encode_push(&Value::Integer(16), span, &mut out).expect("small int");
        encode_push(&Value::Integer(500), span, &mut out).expect("data int");
        encode_push(&Value::Symbol("owner".to_owned()), span, &mut out).expect("symbol");

        assert_eq!(
            out,
            vec![0x00, 0x60, 0x02, 0xf4, 0x01, 0x05, b'o', b'w', b'n', b'e', b'r']
        );
    }

    #[test]
    fn verified_tn12_rejects_gated_preview_records() {
        let ir = IrProgram {
            contracts: vec![IrContract {
                name: "Preview".to_owned(),
                params: Vec::new(),
                finality_depth: Some(1),
                spends: vec![IrSpend {
                    name: "spend".to_owned(),
                    params: Vec::new(),
                    instructions: vec![Instruction::new(
                        Span::new(0, 1),
                        InstructionKind::SequencingCommitment,
                    )],
                }],
            }],
            kip_requirements: vec![15],
            application: kaspascript_model::ApplicationModel::empty(),
        };

        let err = compile_toccata("preview", &ir, "test", Target::VerifiedTn12)
            .expect_err("verified target rejects sequencing");
        assert!(matches!(
            err,
            CodegenError::UnsupportedGrounding { ref id, .. } if id == "sequencing-commitment"
        ));
    }

    #[test]
    fn preview_target_warns_for_gated_kips() {
        let ir = IrProgram {
            contracts: vec![IrContract {
                name: "Preview".to_owned(),
                params: Vec::new(),
                finality_depth: None,
                spends: vec![IrSpend {
                    name: "spend".to_owned(),
                    params: Vec::new(),
                    instructions: vec![Instruction::new(Span::new(0, 1), InstructionKind::Verify)],
                }],
            }],
            kip_requirements: vec![16],
            application: kaspascript_model::ApplicationModel::empty(),
        };

        let artifact =
            compile_toccata("preview", &ir, "test", Target::ToccataPreview).expect("preview");
        assert!(artifact
            .warnings
            .iter()
            .any(|warning| warning.id == "kip-16"));
    }

    #[test]
    fn tn10_toccata_target_warns_for_gated_kips() {
        let ir = IrProgram {
            contracts: vec![IrContract {
                name: "Preview".to_owned(),
                params: Vec::new(),
                finality_depth: None,
                spends: vec![IrSpend {
                    name: "spend".to_owned(),
                    params: Vec::new(),
                    instructions: vec![Instruction::new(Span::new(0, 1), InstructionKind::Verify)],
                }],
            }],
            kip_requirements: vec![20],
            application: kaspascript_model::ApplicationModel::empty(),
        };

        let artifact = compile_toccata("tn10", &ir, "test", Target::Tn10Toccata).expect("tn10");
        assert_eq!(artifact.target, "tn10-toccata");
        assert!(artifact
            .warnings
            .iter()
            .any(|warning| warning.id == "kip-20"));
    }

    #[test]
    fn future_mainnet_rejects_gated_kips_until_sources_are_pinned() {
        let ir = IrProgram {
            contracts: vec![IrContract {
                name: "Future".to_owned(),
                params: Vec::new(),
                finality_depth: None,
                spends: vec![IrSpend {
                    name: "spend".to_owned(),
                    params: Vec::new(),
                    instructions: vec![Instruction::new(Span::new(0, 1), InstructionKind::Verify)],
                }],
            }],
            kip_requirements: vec![20],
            application: kaspascript_model::ApplicationModel::empty(),
        };

        let err = compile_toccata("future", &ir, "test", Target::FutureMainnet)
            .expect_err("future target is locked");
        assert!(matches!(
            err,
            CodegenError::GatedGrounding { ref id, ref target, .. }
                if id == "kip-20" && target == "future-mainnet"
        ));
    }
}
