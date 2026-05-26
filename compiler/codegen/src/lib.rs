//! KaspaScript code generation and artifact creation.

pub mod backends;

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
}
