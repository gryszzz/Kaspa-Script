//! KaspaScript code generation and artifact creation.

pub mod backends;
pub mod grounding;

use grounding::{GroundingWarning, SourceCitation};
use kaspascript_ir::{lower_file, IrError};
use kaspascript_lexer::Span;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Protocol target gate used for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// Only behavior verified against the pinned Kaspa sources is emitted.
    VerifiedTn12,
    /// Preview target for source-recognized but not opcode-verified features.
    ToccataPreview,
    /// Future mainnet target. Gated features fail until mainnet sources are pinned.
    FutureMainnet,
}

impl Target {
    /// Returns the stable artifact label for this target.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerifiedTn12 => "verified-tn12",
            Self::ToccataPreview => "toccata-preview",
            Self::FutureMainnet => "future-mainnet",
        }
    }

    /// Returns true when gated records should be emitted as warnings.
    pub const fn allows_gated_warnings(self) -> bool {
        matches!(self, Self::ToccataPreview)
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Deterministic compiler output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledArtifact {
    pub bytecode: Vec<u8>,
    pub source_hash: [u8; 32],
    pub compiler_version: String,
    pub backend: String,
    pub target: String,
    pub finality_depth: Option<u64>,
    pub kip_requirements: Vec<u16>,
    pub warnings: Vec<GroundingWarning>,
}

/// Code generation error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CodegenError {
    #[error("{0}")]
    Ir(IrError),
    #[error("value too large at byte {}: {}", span.start, message)]
    ValueTooLarge { span: Span, message: String },
    #[error("artifact bytecode is empty")]
    EmptyBytecode,
    #[error("unsupported source-grounding `{id}` from {}: {}", citation.path, message)]
    UnsupportedGrounding {
        id: String,
        citation: SourceCitation,
        message: String,
    },
    #[error("gated source-grounding `{id}` is not allowed for target `{target}` from {}: {}", citation.path, message)]
    GatedGrounding {
        id: String,
        target: String,
        citation: SourceCitation,
        message: String,
    },
    #[error("invalid bytecode at offset {offset}: {message}")]
    InvalidBytecode { offset: usize, message: String },
}

/// Runs IR lowering and code generation for the verified TN12 target.
pub fn compile_file(source: &str, file: &str) -> Result<CompiledArtifact, CodegenError> {
    compile_file_for_target(source, file, Target::VerifiedTn12)
}

/// Runs IR lowering and code generation for an explicit target gate.
pub fn compile_file_for_target(
    source: &str,
    file: &str,
    target: Target,
) -> Result<CompiledArtifact, CodegenError> {
    let ir = lower_file(source, file).map_err(CodegenError::Ir)?;
    backends::toccata::compile_toccata(source, &ir, env!("CARGO_PKG_VERSION"), target)
}

/// Validates a compiled artifact.
pub fn verify_artifact(artifact: &CompiledArtifact) -> Result<(), CodegenError> {
    if artifact.bytecode.is_empty() {
        return Err(CodegenError::EmptyBytecode);
    }
    disassemble(&artifact.bytecode)?;
    Ok(())
}

/// Converts bytecode to lowercase hexadecimal.
pub fn bytecode_hex(bytecode: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytecode.len() * 2);
    for byte in bytecode {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Disassembles bytecode emitted by this backend into stable ASM.
pub fn bytecode_asm(bytecode: &[u8]) -> Result<String, CodegenError> {
    Ok(disassemble(bytecode)?.join(" "))
}

fn disassemble(bytecode: &[u8]) -> Result<Vec<String>, CodegenError> {
    let mut offset = 0usize;
    let mut asm = Vec::new();
    while offset < bytecode.len() {
        let opcode = bytecode[offset];
        offset += 1;

        match opcode {
            0x00 => asm.push("OP_0".to_owned()),
            0x01..=0x4b => {
                let len = opcode as usize;
                let data = read_push_data(bytecode, &mut offset, len)?;
                asm.push(format!("OP_DATA_{len} 0x{}", bytecode_hex(data)));
            }
            0x4c => {
                let len = read_len_u8(bytecode, &mut offset)?;
                let data = read_push_data(bytecode, &mut offset, len)?;
                asm.push(format!("OP_PUSHDATA1 0x{}", bytecode_hex(data)));
            }
            0x4d => {
                let len = read_len_u16(bytecode, &mut offset)?;
                let data = read_push_data(bytecode, &mut offset, len)?;
                asm.push(format!("OP_PUSHDATA2 0x{}", bytecode_hex(data)));
            }
            0x4e => {
                let len = read_len_u32(bytecode, &mut offset)?;
                let data = read_push_data(bytecode, &mut offset, len)?;
                asm.push(format!("OP_PUSHDATA4 0x{}", bytecode_hex(data)));
            }
            0x4f => asm.push("OP_1NEGATE".to_owned()),
            0x51..=0x60 => asm.push(format!("OP_{}", opcode - 0x50)),
            _ => {
                let Some(name) = opcode_name(opcode) else {
                    return Err(CodegenError::InvalidBytecode {
                        offset: offset - 1,
                        message: format!("unknown opcode 0x{opcode:02x}"),
                    });
                };
                asm.push(name.to_owned());
            }
        }
    }
    Ok(asm)
}

fn read_push_data<'a>(
    bytecode: &'a [u8],
    offset: &mut usize,
    len: usize,
) -> Result<&'a [u8], CodegenError> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| CodegenError::InvalidBytecode {
            offset: *offset,
            message: "push length overflow".to_owned(),
        })?;
    if end > bytecode.len() {
        return Err(CodegenError::InvalidBytecode {
            offset: *offset,
            message: "push data exceeds bytecode length".to_owned(),
        });
    }
    let data = &bytecode[*offset..end];
    *offset = end;
    Ok(data)
}

fn read_len_u8(bytecode: &[u8], offset: &mut usize) -> Result<usize, CodegenError> {
    if *offset >= bytecode.len() {
        return Err(CodegenError::InvalidBytecode {
            offset: *offset,
            message: "missing OP_PUSHDATA1 length".to_owned(),
        });
    }
    let len = bytecode[*offset] as usize;
    *offset += 1;
    Ok(len)
}

fn read_len_u16(bytecode: &[u8], offset: &mut usize) -> Result<usize, CodegenError> {
    if bytecode.len().saturating_sub(*offset) < 2 {
        return Err(CodegenError::InvalidBytecode {
            offset: *offset,
            message: "missing OP_PUSHDATA2 length".to_owned(),
        });
    }
    let len = u16::from_le_bytes([bytecode[*offset], bytecode[*offset + 1]]) as usize;
    *offset += 2;
    Ok(len)
}

fn read_len_u32(bytecode: &[u8], offset: &mut usize) -> Result<usize, CodegenError> {
    if bytecode.len().saturating_sub(*offset) < 4 {
        return Err(CodegenError::InvalidBytecode {
            offset: *offset,
            message: "missing OP_PUSHDATA4 length".to_owned(),
        });
    }
    let len = u32::from_le_bytes([
        bytecode[*offset],
        bytecode[*offset + 1],
        bytecode[*offset + 2],
        bytecode[*offset + 3],
    ]) as usize;
    *offset += 4;
    Ok(len)
}

fn opcode_name(opcode: u8) -> Option<&'static str> {
    Some(match opcode {
        0x69 => "OP_VERIFY",
        0x75 => "OP_DROP",
        0x76 => "OP_DUP",
        0x87 => "OP_EQUAL",
        0x91 => "OP_NOT",
        0x93 => "OP_ADD",
        0x94 => "OP_SUB",
        0x9a => "OP_BOOLAND",
        0x9b => "OP_BOOLOR",
        0x9c => "OP_NUMEQUAL",
        0x9e => "OP_NUMNOTEQUAL",
        0x9f => "OP_LESSTHAN",
        0xa0 => "OP_GREATERTHAN",
        0xa1 => "OP_LESSTHANOREQUAL",
        0xa2 => "OP_GREATERTHANOREQUAL",
        0xa8 => "OP_SHA256",
        0xaa => "OP_BLAKE2B",
        0xac => "OP_CHECKSIG",
        0xae => "OP_CHECKMULTISIG",
        0xb0 => "OP_CHECKLOCKTIMEVERIFY",
        0xb1 => "OP_CHECKSEQUENCEVERIFY",
        0xb3 => "OP_TXINPUTCOUNT",
        0xb4 => "OP_TXOUTPUTCOUNT",
        0xb9 => "OP_TXINPUTINDEX",
        0xbe => "OP_TXINPUTAMOUNT",
        0xbf => "OP_TXINPUTSPK",
        0xc2 => "OP_TXOUTPUTAMOUNT",
        0xc3 => "OP_TXOUTPUTSPK",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{artifact_field_records, VerificationStatus};
    use kaspascript_ir::{Instruction, InstructionKind, IrContract, IrProgram, IrSpend};
    use kaspascript_lexer::Span;

    #[test]
    fn compile_is_deterministic() {
        let source = include_str!("../../../tests/contracts/escrow.ks");
        let first = compile_file(source, "escrow.ks").expect("compiles");
        for _ in 0..100 {
            let next = compile_file(source, "escrow.ks").expect("compiles");
            assert_eq!(next.bytecode, first.bytecode);
            assert_eq!(next.source_hash, first.source_hash);
        }
    }

    #[test]
    fn artifact_fields_are_source_grounded_and_warnings_are_emitted() {
        let source = include_str!("../../../tests/contracts/escrow.ks");
        let artifact = compile_file(source, "escrow.ks").expect("compiles");

        assert!(!artifact.bytecode.is_empty());
        assert_eq!(artifact.backend, "kaspa-txscript");
        assert_eq!(artifact.target, Target::VerifiedTn12.as_str());
        assert_eq!(artifact.compiler_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(artifact.finality_depth, Some(10));
        assert!(artifact.kip_requirements.contains(&10));
        assert!(artifact.warnings.is_empty());

        for record in artifact_field_records() {
            assert_eq!(record.status, VerificationStatus::Verified);
            assert_eq!(record.citation.path, "compiler/codegen/src/lib.rs");
        }
    }

    #[test]
    fn unsupported_builtin_fails_compilation() {
        let source = r#"
            contract UnsupportedHash {
              params { secret: Bytes, expected: Hash }
              spend claim() {
                require hash160(secret) == expected;
              }
            }
        "#;

        let error = compile_file(source, "unsupported.ks").expect_err("unsupported hash160");
        assert!(matches!(
            error,
            CodegenError::UnsupportedGrounding { ref id, .. } if id == "hash160"
        ));
    }

    #[test]
    fn unsupported_ir_instruction_fails_before_bytecode_emission() {
        let ir = IrProgram {
            contracts: vec![IrContract {
                name: "Manual".to_owned(),
                finality_depth: None,
                spends: vec![IrSpend {
                    name: "s".to_owned(),
                    instructions: vec![Instruction {
                        span: Span::new(0, 1),
                        kind: InstructionKind::ZkVerifyRiscZero,
                    }],
                }],
            }],
            kip_requirements: vec![16],
        };

        let error =
            backends::toccata::compile_toccata("manual", &ir, "test", Target::ToccataPreview)
                .expect_err("unsupported risczero");
        assert!(matches!(
            error,
            CodegenError::UnsupportedGrounding { ref id, .. } if id == "zk-risczero-verify"
        ));
    }

    #[test]
    fn disassembles_canonical_pushes_and_known_opcodes() {
        let bytecode = vec![0x00, 0x51, 0x02, b'k', b's', 0xaa, 0x69];
        let asm = bytecode_asm(&bytecode).expect("asm");
        assert_eq!(asm, "OP_0 OP_1 OP_DATA_2 0x6b73 OP_BLAKE2B OP_VERIFY");
        assert_eq!(bytecode_hex(&bytecode), "0051026b73aa69");
    }
}
