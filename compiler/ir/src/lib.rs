//! KaspaScript opcode-agnostic IR.

mod application;
pub mod gen;
pub mod instructions;
pub mod types;

pub use gen::{lower, lower_file, lower_program, IrContract, IrError, IrProgram, IrSpend};
pub use instructions::{Instruction, InstructionKind};
pub use types::Value;
