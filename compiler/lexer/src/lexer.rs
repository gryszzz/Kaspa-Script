use std::fmt;

use logos::Logos;
use thiserror::Error;

use crate::token::{SourceLocation, Span, Token, TokenKind, TypeName};

/// Lexer error with file, line, and column.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct LexError {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
    pub message: String,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum LogosError {
    #[default]
    Unexpected,
    InvalidEscape(char),
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Logos, Debug, Clone, PartialEq, Eq)]
#[logos(error = LogosError)]
enum RawToken {
    #[regex(r"[ \t\r\n\f]+", logos::skip)]
    #[regex(r"//[^\n]*", logos::skip)]
    #[regex(r"/\*([^*]|\*[^/])*\*/", logos::skip)]
    Skip,

    #[token("contract")]
    Contract,
    #[token("params")]
    Params,
    #[token("spend")]
    Spend,
    #[token("require")]
    Require,
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("return")]
    Return,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("covenant")]
    Covenant,
    #[token("covenant_id")]
    CovenantId,
    #[token("zk_verify")]
    ZkVerify,
    #[token("finality_depth")]
    FinalityDepth,
    #[token("sequencing")]
    Sequencing,
    #[token("multisig")]
    Multisig,
    #[token("input")]
    InputBuiltin,
    #[token("output")]
    OutputBuiltin,
    #[token("block")]
    Block,

    #[token("PublicKey")]
    PublicKey,
    #[token("Signature")]
    Signature,
    #[token("Hash")]
    Hash,
    #[token("BlockHeight")]
    BlockHeight,
    #[token("Amount")]
    Amount,
    #[token("Bool")]
    Bool,
    #[token("Bytes")]
    Bytes,
    #[token("CovenantID")]
    CovenantID,
    #[token("ZKProof")]
    ZKProof,
    #[token("UTXO")]
    UTXO,
    #[token("Output")]
    Output,
    #[token("Input")]
    Input,

    #[token("==")]
    EqualEqual,
    #[token("!=")]
    BangEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("<=")]
    LessEqual,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("->")]
    Arrow,
    #[token("=")]
    Equal,
    #[token("!")]
    Bang,
    #[token(">")]
    Greater,
    #[token("<")]
    Less,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("(")]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token(".")]
    Dot,

    #[regex(r"[0-9]+", |lex| lex.slice().to_owned())]
    Integer(String),
    #[regex(r#""([^"\\]|\\["\\/nrt])*""#, decode_string)]
    String(String),
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", |lex| lex.slice().to_owned())]
    Identifier(String),
}

/// Tokenizes source using `<source>` as the file name.
pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    lex_file(source, "<source>")
}

/// Tokenizes source using the provided file name in diagnostics.
pub fn lex_file(source: &str, file: &str) -> Result<Vec<Token>, LexError> {
    detect_unterminated_block_comment(source, file)?;
    detect_unterminated_string(source, file)?;

    let mut tokens = Vec::new();
    let mut lexer = RawToken::lexer(source);

    while let Some(raw) = lexer.next() {
        let range = lexer.span();
        let span = Span::new(range.start, range.end);
        let kind = match raw {
            Ok(raw) => raw.into_token_kind(),
            Err(LogosError::InvalidEscape(ch)) => {
                return Err(error_at(
                    source,
                    file,
                    span,
                    format!("invalid escape sequence \\{ch}"),
                ));
            }
            Err(LogosError::Unexpected) => {
                let ch = source[span.start..span.end].chars().next().unwrap_or('\0');
                return Err(error_at(
                    source,
                    file,
                    span,
                    format!("unexpected character `{ch}`"),
                ));
            }
        };
        if let Some(kind) = kind {
            tokens.push(Token::new(kind, span));
        }
    }

    Ok(tokens)
}

/// Resolves a byte offset into a source location.
pub fn locate(source: &str, file: &str, offset: usize) -> SourceLocation {
    let mut line = 1;
    let mut column = 1;

    for (byte, ch) in source.char_indices() {
        if byte >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    SourceLocation {
        file: file.to_owned(),
        line,
        column,
    }
}

impl RawToken {
    fn into_token_kind(self) -> Option<TokenKind> {
        match self {
            RawToken::Skip => None,
            RawToken::Contract => Some(TokenKind::Contract),
            RawToken::Params => Some(TokenKind::Params),
            RawToken::Spend => Some(TokenKind::Spend),
            RawToken::Require => Some(TokenKind::Require),
            RawToken::Let => Some(TokenKind::Let),
            RawToken::If => Some(TokenKind::If),
            RawToken::Else => Some(TokenKind::Else),
            RawToken::Return => Some(TokenKind::Return),
            RawToken::True => Some(TokenKind::True),
            RawToken::False => Some(TokenKind::False),
            RawToken::Covenant => Some(TokenKind::Covenant),
            RawToken::CovenantId => Some(TokenKind::CovenantId),
            RawToken::ZkVerify => Some(TokenKind::ZkVerify),
            RawToken::FinalityDepth => Some(TokenKind::FinalityDepth),
            RawToken::Sequencing => Some(TokenKind::Sequencing),
            RawToken::Multisig => Some(TokenKind::Multisig),
            RawToken::InputBuiltin => Some(TokenKind::InputBuiltin),
            RawToken::OutputBuiltin => Some(TokenKind::OutputBuiltin),
            RawToken::Block => Some(TokenKind::Block),
            RawToken::PublicKey => Some(TokenKind::Type(TypeName::PublicKey)),
            RawToken::Signature => Some(TokenKind::Type(TypeName::Signature)),
            RawToken::Hash => Some(TokenKind::Type(TypeName::Hash)),
            RawToken::BlockHeight => Some(TokenKind::Type(TypeName::BlockHeight)),
            RawToken::Amount => Some(TokenKind::Type(TypeName::Amount)),
            RawToken::Bool => Some(TokenKind::Type(TypeName::Bool)),
            RawToken::Bytes => Some(TokenKind::Type(TypeName::Bytes)),
            RawToken::CovenantID => Some(TokenKind::Type(TypeName::CovenantID)),
            RawToken::ZKProof => Some(TokenKind::Type(TypeName::ZKProof)),
            RawToken::UTXO => Some(TokenKind::Type(TypeName::UTXO)),
            RawToken::Output => Some(TokenKind::Type(TypeName::Output)),
            RawToken::Input => Some(TokenKind::Type(TypeName::Input)),
            RawToken::Equal => Some(TokenKind::Equal),
            RawToken::EqualEqual => Some(TokenKind::EqualEqual),
            RawToken::Bang => Some(TokenKind::Bang),
            RawToken::BangEqual => Some(TokenKind::BangEqual),
            RawToken::Greater => Some(TokenKind::Greater),
            RawToken::GreaterEqual => Some(TokenKind::GreaterEqual),
            RawToken::Less => Some(TokenKind::Less),
            RawToken::LessEqual => Some(TokenKind::LessEqual),
            RawToken::AndAnd => Some(TokenKind::AndAnd),
            RawToken::OrOr => Some(TokenKind::OrOr),
            RawToken::Plus => Some(TokenKind::Plus),
            RawToken::Minus => Some(TokenKind::Minus),
            RawToken::Star => Some(TokenKind::Star),
            RawToken::Slash => Some(TokenKind::Slash),
            RawToken::Percent => Some(TokenKind::Percent),
            RawToken::Arrow => Some(TokenKind::Arrow),
            RawToken::LeftBrace => Some(TokenKind::LeftBrace),
            RawToken::RightBrace => Some(TokenKind::RightBrace),
            RawToken::LeftParen => Some(TokenKind::LeftParen),
            RawToken::RightParen => Some(TokenKind::RightParen),
            RawToken::LeftBracket => Some(TokenKind::LeftBracket),
            RawToken::RightBracket => Some(TokenKind::RightBracket),
            RawToken::Comma => Some(TokenKind::Comma),
            RawToken::Colon => Some(TokenKind::Colon),
            RawToken::Semicolon => Some(TokenKind::Semicolon),
            RawToken::Dot => Some(TokenKind::Dot),
            RawToken::Integer(value) => Some(TokenKind::Integer(value)),
            RawToken::String(value) => Some(TokenKind::String(value)),
            RawToken::Identifier(value) => Some(TokenKind::Identifier(value)),
        }
    }
}

fn decode_string(lexer: &mut logos::Lexer<'_, RawToken>) -> Result<String, LogosError> {
    let slice = lexer.slice();
    let inner = &slice[1..slice.len() - 1];
    let mut decoded = String::new();
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(LogosError::InvalidEscape('\\'));
        };
        match escaped {
            '"' => decoded.push('"'),
            '\\' => decoded.push('\\'),
            '/' => decoded.push('/'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            other => return Err(LogosError::InvalidEscape(other)),
        }
    }

    Ok(decoded)
}

fn detect_unterminated_block_comment(source: &str, file: &str) -> Result<(), LexError> {
    let mut cursor = 0;
    while let Some(relative_start) = source[cursor..].find("/*") {
        let start = cursor + relative_start;
        let search_from = start + 2;
        let Some(relative_end) = source[search_from..].find("*/") else {
            return Err(error_at(
                source,
                file,
                Span::new(start, start + 2),
                "unterminated block comment".to_owned(),
            ));
        };
        cursor = search_from + relative_end + 2;
    }
    Ok(())
}

fn detect_unterminated_string(source: &str, file: &str) -> Result<(), LexError> {
    let mut in_string = false;
    let mut string_start = 0;
    let mut escaped = false;

    for (offset, ch) in source.char_indices() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                string_start = offset;
            }
            continue;
        }

        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_string = false,
            '\n' => {
                return Err(error_at(
                    source,
                    file,
                    Span::new(string_start, string_start + 1),
                    "unterminated string".to_owned(),
                ));
            }
            _ => {}
        }
    }

    if in_string {
        return Err(error_at(
            source,
            file,
            Span::new(string_start, string_start + 1),
            "unterminated string".to_owned(),
        ));
    }

    Ok(())
}

fn error_at(source: &str, file: &str, span: Span, message: String) -> LexError {
    let location = locate(source, file, span.start);
    LexError {
        file: location.file,
        line: location.line,
        column: location.column,
        span,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_keywords_types_and_positions() {
        let tokens = lex("contract Vault { owner: PublicKey }").expect("lexes");
        assert_eq!(tokens[0].kind, TokenKind::Contract);
        assert_eq!(tokens[0].span, Span::new(0, 8));
        assert_eq!(tokens[5].kind, TokenKind::Type(TypeName::PublicKey));
    }

    #[test]
    fn skips_comments() {
        let tokens = lex("contract /* block */ A // line\n{}").expect("lexes");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("A".to_owned()));
    }

    #[test]
    fn reports_unexpected_character_with_line_col() {
        let error = lex_file("contract\n@", "bad.ks").expect_err("fails");
        assert_eq!(error.to_string(), "bad.ks:2:1: unexpected character `@`");
    }

    #[test]
    fn reports_unterminated_block_comment() {
        let error = lex_file("/* nope", "bad.ks").expect_err("fails");
        assert_eq!(error.to_string(), "bad.ks:1:1: unterminated block comment");
    }
}
