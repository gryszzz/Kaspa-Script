use sha2::{Digest, Sha256};

use kaspascript_ir::{Instruction, InstructionKind, IrProgram, Value};
use kaspascript_lexer::Span;

use crate::{CodegenError, CompiledArtifact};

const OP_PUSH_INT: u8 = 0x01;
const OP_PUSH_BOOL: u8 = 0x02;
const OP_PUSH_BYTES: u8 = 0x03;
const OP_DROP: u8 = 0x75;
const OP_DUP: u8 = 0x76;
const OP_EQUAL: u8 = 0x87;
const OP_VERIFY: u8 = 0x69;
const OP_CHECKSIG: u8 = 0xac;
const OP_CHECKMULTISIG: u8 = 0xae;
const OP_CHECKLOCKTIMEVERIFY: u8 = 0xb1;
const OP_ADD: u8 = 0x93;
const OP_SUB: u8 = 0x94;
const OP_MUL: u8 = 0x95;
const OP_DIV: u8 = 0x96;
const OP_MOD: u8 = 0x97;
const OP_GREATERTHAN: u8 = 0xa0;
const OP_GREATERTHANOREQUAL: u8 = 0xa2;
const OP_NOT: u8 = 0x91;
const OP_AND: u8 = 0x9a;
const OP_OR: u8 = 0x9b;
const OP_SHA256: u8 = 0xa8;
const OP_HASH160: u8 = 0xa9;
const OP_BLAKE2B: u8 = 0xc0;
const OP_INPUTVALUE: u8 = 0xd0;
const OP_INPUTSCRIPT: u8 = 0xd1;
const OP_OUTPUTVALUE: u8 = 0xd2;
const OP_OUTPUTSCRIPT: u8 = 0xd3;
const OP_INPUTCOUNT: u8 = 0xd4;
const OP_OUTPUTCOUNT: u8 = 0xd5;
const OP_COVENANTID: u8 = 0xe0;
const OP_COVENANTID_DEPTH: u8 = 0xe1;
const OP_ZK_GROTH16_VERIFY: u8 = 0xf0;
const OP_ZK_RISCZERO_VERIFY: u8 = 0xf1;
const OP_SEQUENCING_COMMITMENT: u8 = 0xf2;
const OP_CHECK_HASH_PREIMAGE: u8 = 0xf3;

/// Compiles IR into deterministic Toccata-target bytecode.
pub fn compile_toccata(
    source: &str,
    ir: &IrProgram,
    compiler_version: &str,
) -> Result<CompiledArtifact, CodegenError> {
    let mut bytecode = Vec::new();
    for contract in &ir.contracts {
        for spend in &contract.spends {
            for instruction in &spend.instructions {
                encode_instruction(instruction, &mut bytecode)?;
            }
        }
    }

    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let source_hash = hasher.finalize().into();

    Ok(CompiledArtifact {
        bytecode,
        source_hash,
        compiler_version: compiler_version.to_owned(),
        backend: "toccata".to_owned(),
        finality_depth: ir
            .contracts
            .iter()
            .filter_map(|contract| contract.finality_depth)
            .max(),
        kip_requirements: ir.kip_requirements.clone(),
    })
}

fn encode_instruction(instruction: &Instruction, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    match &instruction.kind {
        InstructionKind::Push(value) => encode_push(value, instruction.span, out)?,
        InstructionKind::Drop => out.push(OP_DROP),
        InstructionKind::Dup => out.push(OP_DUP),
        InstructionKind::InputValue(index) => encode_indexed(OP_INPUTVALUE, *index, out),
        InstructionKind::InputScript(index) => encode_indexed(OP_INPUTSCRIPT, *index, out),
        InstructionKind::OutputValue(index) => encode_indexed(OP_OUTPUTVALUE, *index, out),
        InstructionKind::OutputScript(index) => encode_indexed(OP_OUTPUTSCRIPT, *index, out),
        InstructionKind::OutputCount => out.push(OP_OUTPUTCOUNT),
        InstructionKind::InputCount => out.push(OP_INPUTCOUNT),
        InstructionKind::CheckSig { key_slot } => encode_indexed(OP_CHECKSIG, *key_slot, out),
        InstructionKind::CheckMultiSig {
            required,
            key_count,
        } => {
            out.push(OP_CHECKMULTISIG);
            out.extend(required.to_le_bytes());
            out.extend(key_count.to_le_bytes());
        }
        InstructionKind::CheckLockHeight(height) => {
            out.push(OP_CHECKLOCKTIMEVERIFY);
            out.extend(height.to_le_bytes());
        }
        InstructionKind::CheckLockTime(time) => {
            out.push(OP_CHECKLOCKTIMEVERIFY);
            out.extend(time.to_le_bytes());
        }
        InstructionKind::CovenantDepth => out.push(OP_COVENANTID_DEPTH),
        InstructionKind::CovenantId => out.push(OP_COVENANTID),
        InstructionKind::ZkVerifyGroth16 => out.push(OP_ZK_GROTH16_VERIFY),
        InstructionKind::ZkVerifyRiscZero => out.push(OP_ZK_RISCZERO_VERIFY),
        InstructionKind::SequencingCommitment => out.push(OP_SEQUENCING_COMMITMENT),
        InstructionKind::Sha256 => out.push(OP_SHA256),
        InstructionKind::Blake2b => out.push(OP_BLAKE2B),
        InstructionKind::Hash160 => out.push(OP_HASH160),
        InstructionKind::CheckHashPreimage => out.push(OP_CHECK_HASH_PREIMAGE),
        InstructionKind::Verify => out.push(OP_VERIFY),
        InstructionKind::Equal => out.push(OP_EQUAL),
        InstructionKind::NotEqual => {
            out.push(OP_EQUAL);
            out.push(OP_NOT);
        }
        InstructionKind::GreaterThan => out.push(OP_GREATERTHAN),
        InstructionKind::GreaterThanOrEqual => out.push(OP_GREATERTHANOREQUAL),
        InstructionKind::And => out.push(OP_AND),
        InstructionKind::Or => out.push(OP_OR),
        InstructionKind::Not => out.push(OP_NOT),
        InstructionKind::Add => out.push(OP_ADD),
        InstructionKind::Sub => out.push(OP_SUB),
        InstructionKind::Mul => out.push(OP_MUL),
        InstructionKind::Div => out.push(OP_DIV),
        InstructionKind::Mod => out.push(OP_MOD),
    }
    Ok(())
}

fn encode_push(value: &Value, span: Span, out: &mut Vec<u8>) -> Result<(), CodegenError> {
    match value {
        Value::Integer(value) => {
            out.push(OP_PUSH_INT);
            out.extend(value.to_le_bytes());
        }
        Value::Bool(value) => {
            out.push(OP_PUSH_BOOL);
            out.push(u8::from(*value));
        }
        Value::String(value) | Value::Symbol(value) => {
            let bytes = value.as_bytes();
            let len = u32::try_from(bytes.len()).map_err(|_| CodegenError::ValueTooLarge {
                span,
                message: "push value exceeds u32 length".to_owned(),
            })?;
            out.push(OP_PUSH_BYTES);
            out.extend(len.to_le_bytes());
            out.extend(bytes);
        }
        Value::Type(ty) => {
            out.push(OP_PUSH_BYTES);
            let text = format!("{ty:?}");
            let bytes = text.as_bytes();
            let len = u32::try_from(bytes.len()).map_err(|_| CodegenError::ValueTooLarge {
                span,
                message: "type push value exceeds u32 length".to_owned(),
            })?;
            out.extend(len.to_le_bytes());
            out.extend(bytes);
        }
    }
    Ok(())
}

fn encode_indexed(opcode: u8, index: u32, out: &mut Vec<u8>) {
    out.push(opcode);
    out.extend(index.to_le_bytes());
}
