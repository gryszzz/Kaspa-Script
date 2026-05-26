//! KaspaScript lexer.

pub mod lexer;
pub mod token;

pub use lexer::{lex, lex_file, locate, LexError};
pub use token::{SourceLocation, Span, Token, TokenKind, TypeName};
