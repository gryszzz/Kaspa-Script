//! KaspaScript parser.

pub mod ast;
pub mod parser;

pub use ast::{BinaryOp, Contract, Expr, Ident, Param, Program, Spend, Stmt, UnaryOp};
pub use parser::{parse, parse_file, ParseError, Parser};
