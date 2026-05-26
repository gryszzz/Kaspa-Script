mod lexer;

pub use lexer::{
    lex, Keyword, LexError, LexErrorKind, Lexer, Position, Span, Token, TokenKind, TypeName,
};
