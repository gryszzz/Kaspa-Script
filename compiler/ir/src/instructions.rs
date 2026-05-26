use kaspascript_lexer::Span;
use serde::{Deserialize, Serialize};

use crate::types::Value;

/// Opcode-agnostic IR instruction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub span: Span,
    pub kind: InstructionKind,
}

/// KaspaScript IR instruction kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstructionKind {
    Push(Value),
    Drop,
    Dup,
    InputValue(u32),
    InputScript(u32),
    OutputValue(u32),
    OutputScript(u32),
    OutputCount,
    InputCount,
    CheckSig { key_slot: u32 },
    CheckMultiSig { required: u32, key_count: u32 },
    CheckLockHeight(u64),
    CheckLockTime(u64),
    CheckLockHeightFromStack,
    CheckLockTimeFromStack,
    CovenantDepth,
    CovenantId,
    ZkVerifyGroth16,
    ZkVerifyRiscZero,
    SequencingCommitment,
    Sha256,
    Blake2b,
    Hash160,
    CheckHashPreimage,
    Verify,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl Instruction {
    /// Creates an instruction.
    pub const fn new(span: Span, kind: InstructionKind) -> Self {
        Self { span, kind }
    }
}
