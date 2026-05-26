use kaspascript_lexer::{Span, TypeName};
use serde::{Deserialize, Serialize};

/// Complete KaspaScript source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub contracts: Vec<Contract>,
}

/// Contract declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub name: Ident,
    pub params: Vec<Param>,
    pub finality_depth: Option<u64>,
    pub spends: Vec<Spend>,
    pub span: Span,
}

/// Identifier with source span.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// Contract parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: Ident,
    pub ty: TypeName,
    pub span: Span,
}

/// Spend function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    pub name: Ident,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// Spend body statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stmt {
    Let { name: Ident, expr: Expr, span: Span },
    Require { expr: Expr, span: Span },
    Return { expr: Expr, span: Span },
}

impl Stmt {
    /// Returns the statement span.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. } | Stmt::Require { span, .. } | Stmt::Return { span, .. } => {
                *span
            }
        }
    }
}

/// Expression AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expr {
    Ident(Ident),
    Integer {
        value: u64,
        span: Span,
    },
    String {
        value: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    Array {
        elements: Vec<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Field {
        object: Box<Expr>,
        field: Ident,
        span: Span,
    },
}

impl Expr {
    /// Returns the expression span.
    pub fn span(&self) -> Span {
        match self {
            Expr::Ident(ident) => ident.span,
            Expr::Integer { span, .. }
            | Expr::String { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Array { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Field { span, .. } => *span,
        }
    }
}

/// Unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not,
    Negate,
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}
