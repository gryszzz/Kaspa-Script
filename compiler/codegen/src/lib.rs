//! KaspaScript code generation and artifact creation.

pub mod backends;
pub mod grounding;

use grounding::{GroundingWarning, SourceCitation};
use kaspascript_ir::{lower_file, IrError};
use kaspascript_lexer::Span;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Deterministic compiler output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledArtifact {
    pub bytecode: Vec<u8>,
    pub source_hash: [u8; 32],
    pub compiler_version: String,
    pub backend: String,
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
}

/// Runs IR lowering and Toccata code generation.
pub fn compile_file(source: &str, file: &str) -> Result<CompiledArtifact, CodegenError> {
    let ir = lower_file(source, file).map_err(CodegenError::Ir)?;
    backends::toccata::compile_toccata(source, &ir, env!("CARGO_PKG_VERSION"))
}

/// Validates a compiled artifact.
pub fn verify_artifact(artifact: &CompiledArtifact) -> Result<(), CodegenError> {
    if artifact.bytecode.is_empty() {
        return Err(CodegenError::EmptyBytecode);
    }
    Ok(())
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
        assert_eq!(artifact.backend, "toccata");
        assert_eq!(artifact.compiler_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(artifact.finality_depth, Some(10));
        assert!(artifact.kip_requirements.contains(&17));
        assert!(artifact
            .warnings
            .iter()
            .any(|warning| warning.id == "kip-17"));
        assert!(artifact
            .warnings
            .iter()
            .any(|warning| warning.citation.path == "compiler/codegen/src/backends/toccata.rs"));

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

        let error = backends::toccata::compile_toccata("manual", &ir, "test")
            .expect_err("unsupported risczero");
        assert!(matches!(
            error,
            CodegenError::UnsupportedGrounding { ref id, .. } if id == "zk-risczero-verify"
        ));
    }
}
