use serde::{Deserialize, Serialize};

/// Byte span within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Creates a new byte span.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A source location resolved from a byte offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// KaspaScript primitive and composite type names.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeName {
    PublicKey,
    Signature,
    Hash,
    BlockHeight,
    Amount,
    Bool,
    Bytes,
    CovenantID,
    ZKProof,
    UTXO,
    Output,
    Input,
}

/// KaspaScript token kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    Contract,
    Params,
    Spend,
    Require,
    Let,
    If,
    Else,
    Return,
    True,
    False,
    Covenant,
    CovenantId,
    ZkVerify,
    FinalityDepth,
    Sequencing,
    Multisig,
    InputBuiltin,
    OutputBuiltin,
    Block,
    Type(TypeName),
    Identifier(String),
    Integer(String),
    String(String),
    Equal,
    EqualEqual,
    Bang,
    BangEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    AndAnd,
    OrOr,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Arrow,
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
}

/// A token and its byte span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    /// Creates a token.
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}
