//! KaspaScript semantic analyzer.

pub mod checker;
pub mod scope;

pub use checker::{
    analyze, analyze_file, analyze_program, Analysis, AnalyzeError, AnalyzeFailure, SemanticError,
};
pub use scope::Scope;
