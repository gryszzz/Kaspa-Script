use std::fmt;

use kaspascript_lexer::{lex_file, locate, LexError, SourceLocation, Span, Token, TokenKind};
use thiserror::Error;

use crate::ast::{BinaryOp, Contract, Expr, Ident, Param, Program, Spend, Stmt, UnaryOp};

/// Parser error with source location.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub span: Span,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.file, self.line, self.column, self.message
        )
    }
}

/// Parses a KaspaScript source file.
pub fn parse_file(source: &str, file: &str) -> Result<Program, ParseError> {
    let tokens = lex_file(source, file).map_err(ParseError::from)?;
    Parser::new(tokens, source, file).parse_program()
}

/// Parses source using `<source>` as the file name.
pub fn parse(source: &str) -> Result<Program, ParseError> {
    parse_file(source, "<source>")
}

/// Pratt parser for KaspaScript.
pub struct Parser<'source> {
    tokens: Vec<Token>,
    source: &'source str,
    file: &'source str,
    cursor: usize,
}

impl<'source> Parser<'source> {
    /// Creates a parser from tokens.
    pub fn new(tokens: Vec<Token>, source: &'source str, file: &'source str) -> Self {
        Self {
            tokens,
            source,
            file,
            cursor: 0,
        }
    }

    /// Parses the complete token stream.
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut contracts = Vec::new();
        while !self.is_at_end() {
            contracts.push(self.parse_contract()?);
        }
        Ok(Program { contracts })
    }

    fn parse_contract(&mut self) -> Result<Contract, ParseError> {
        let start = self
            .expect_exact(TokenKind::Contract, "`contract`")?
            .span
            .start;
        let name = self.parse_identifier()?;
        self.expect_exact(TokenKind::LeftBrace, "`{`")?;

        let mut params = Vec::new();
        let mut finality_depth = None;
        let mut spends = Vec::new();

        while !self.check_exact(&TokenKind::RightBrace) {
            if self.is_at_end() {
                return Err(self.error_here("expected `}`"));
            }
            if self.match_exact(TokenKind::Params) {
                let parsed = self.parse_params_block()?;
                params.extend(parsed.0);
                if parsed.1.is_some() {
                    finality_depth = parsed.1;
                }
            } else if self.match_exact(TokenKind::Spend) {
                spends.push(self.parse_spend()?);
            } else {
                return Err(self.error_here("expected `params`, `spend`, or `}`"));
            }
        }
        let end = self.expect_exact(TokenKind::RightBrace, "`}`")?.span.end;

        Ok(Contract {
            name,
            params,
            finality_depth,
            spends,
            span: Span::new(start, end),
        })
    }

    fn parse_params_block(&mut self) -> Result<(Vec<Param>, Option<u64>), ParseError> {
        self.expect_exact(TokenKind::LeftBrace, "`{`")?;
        let mut params = Vec::new();
        let mut finality_depth = None;

        while !self.check_exact(&TokenKind::RightBrace) {
            let name = self.parse_identifier()?;
            self.expect_exact(TokenKind::Colon, "`:`")?;

            if name.name == "finality_depth" {
                let token = self.advance_or_error("expected integer literal")?;
                let value = match token.kind {
                    TokenKind::Integer(value) => self.parse_u64(&value, token.span)?,
                    _ => return Err(self.error_at(token.span, "expected integer literal")),
                };
                finality_depth = Some(value);
            } else {
                let ty = match self.advance_or_error("expected type")? {
                    Token {
                        kind: TokenKind::Type(ty),
                        ..
                    } => ty,
                    token => return Err(self.error_at(token.span, "expected type")),
                };
                let span = Span::new(name.span.start, self.previous_span().end);
                params.push(Param { name, ty, span });
            }

            if !self.match_exact(TokenKind::Comma) && !self.check_exact(&TokenKind::RightBrace) {
                return Err(self.error_here("expected `,` or `}`"));
            }
        }

        self.expect_exact(TokenKind::RightBrace, "`}`")?;
        Ok((params, finality_depth))
    }

    fn parse_spend(&mut self) -> Result<Spend, ParseError> {
        let name = self.parse_identifier()?;
        let start = name.span.start;
        self.expect_exact(TokenKind::LeftParen, "`(`")?;
        let params = self.parse_typed_params(TokenKind::RightParen)?;
        self.expect_exact(TokenKind::RightParen, "`)`")?;
        self.expect_exact(TokenKind::LeftBrace, "`{`")?;

        let mut body = Vec::new();
        while !self.check_exact(&TokenKind::RightBrace) {
            if self.is_at_end() {
                return Err(self.error_here("expected `}`"));
            }
            body.push(self.parse_statement()?);
        }
        let end = self.expect_exact(TokenKind::RightBrace, "`}`")?.span.end;

        Ok(Spend {
            name,
            params,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_typed_params(&mut self, terminator: TokenKind) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        while !self.check_exact(&terminator) {
            let name = self.parse_identifier()?;
            self.expect_exact(TokenKind::Colon, "`:`")?;
            let token = self.advance_or_error("expected type")?;
            let ty = match token.kind {
                TokenKind::Type(ty) => ty,
                _ => return Err(self.error_at(token.span, "expected type")),
            };
            params.push(Param {
                span: Span::new(name.span.start, token.span.end),
                name,
                ty,
            });
            if !self.match_exact(TokenKind::Comma) && !self.check_exact(&terminator) {
                return Err(self.error_here("expected `,` or terminator"));
            }
        }
        Ok(params)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.match_exact(TokenKind::Let) {
            let start = self.previous_span().start;
            let name = self.parse_identifier()?;
            self.expect_exact(TokenKind::Equal, "`=`")?;
            let expr = self.parse_expression()?;
            let end = self.expect_exact(TokenKind::Semicolon, "`;`")?.span.end;
            return Ok(Stmt::Let {
                name,
                expr,
                span: Span::new(start, end),
            });
        }

        if self.match_exact(TokenKind::Require) {
            let start = self.previous_span().start;
            let expr = self.parse_expression()?;
            let end = self.expect_exact(TokenKind::Semicolon, "`;`")?.span.end;
            return Ok(Stmt::Require {
                expr,
                span: Span::new(start, end),
            });
        }

        if self.match_exact(TokenKind::Return) {
            let start = self.previous_span().start;
            let expr = self.parse_expression()?;
            let end = self.expect_exact(TokenKind::Semicolon, "`;`")?.span.end;
            return Ok(Stmt::Return {
                expr,
                span: Span::new(start, end),
            });
        }

        Err(self.error_here("expected statement"))
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_precedence(0)
    }

    fn parse_precedence(&mut self, min_precedence: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some((op, precedence)) = self.current_binary_op() {
            if precedence < min_precedence {
                break;
            }
            self.cursor += 1;
            let right = self.parse_precedence(precedence + 1)?;
            let span = Span::new(left.span().start, right.span().end);
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.match_exact(TokenKind::Bang) {
            let start = self.previous_span().start;
            let expr = self.parse_unary()?;
            let span = Span::new(start, expr.span().end);
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span,
            });
        }
        if self.match_exact(TokenKind::Minus) {
            let start = self.previous_span().start;
            let expr = self.parse_unary()?;
            let span = Span::new(start, expr.span().end);
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(expr),
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.match_exact(TokenKind::LeftParen) {
                let args = self.parse_arguments()?;
                let end = self.expect_exact(TokenKind::RightParen, "`)`")?.span.end;
                expr = Expr::Call {
                    span: Span::new(expr.span().start, end),
                    callee: Box::new(expr),
                    args,
                };
                continue;
            }
            if self.match_exact(TokenKind::Dot) {
                let field = self.parse_identifier()?;
                expr = Expr::Field {
                    span: Span::new(expr.span().start, field.span.end),
                    object: Box::new(expr),
                    field,
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !self.check_exact(&TokenKind::RightParen) {
            args.push(self.parse_expression()?);
            if !self.match_exact(TokenKind::Comma) && !self.check_exact(&TokenKind::RightParen) {
                return Err(self.error_here("expected `,` or `)`"));
            }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance_or_error("expected expression")?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(Expr::Ident(Ident {
                name,
                span: token.span,
            })),
            kind if is_value_keyword(&kind) => Ok(Expr::Ident(Ident {
                name: token_text(&kind).to_owned(),
                span: token.span,
            })),
            TokenKind::Integer(value) => Ok(Expr::Integer {
                value: self.parse_u64(&value, token.span)?,
                span: token.span,
            }),
            TokenKind::String(value) => Ok(Expr::String {
                value,
                span: token.span,
            }),
            TokenKind::True => Ok(Expr::Bool {
                value: true,
                span: token.span,
            }),
            TokenKind::False => Ok(Expr::Bool {
                value: false,
                span: token.span,
            }),
            TokenKind::LeftParen => {
                let expr = self.parse_expression()?;
                self.expect_exact(TokenKind::RightParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::LeftBracket => self.parse_array(token.span.start),
            _ => Err(self.error_at(token.span, "expected expression")),
        }
    }

    fn parse_array(&mut self, start: usize) -> Result<Expr, ParseError> {
        let mut elements = Vec::new();
        while !self.check_exact(&TokenKind::RightBracket) {
            elements.push(self.parse_expression()?);
            if !self.match_exact(TokenKind::Comma) && !self.check_exact(&TokenKind::RightBracket) {
                return Err(self.error_here("expected `,` or `]`"));
            }
        }
        let end = self.expect_exact(TokenKind::RightBracket, "`]`")?.span.end;
        Ok(Expr::Array {
            elements,
            span: Span::new(start, end),
        })
    }

    fn parse_identifier(&mut self) -> Result<Ident, ParseError> {
        let token = self.advance_or_error("expected identifier")?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(Ident {
                name,
                span: token.span,
            }),
            kind if is_value_keyword(&kind) => Ok(Ident {
                name: token_text(&kind).to_owned(),
                span: token.span,
            }),
            _ => Err(self.error_at(token.span, "expected identifier")),
        }
    }

    fn current_binary_op(&self) -> Option<(BinaryOp, u8)> {
        match self.current_kind()? {
            TokenKind::OrOr => Some((BinaryOp::Or, 1)),
            TokenKind::AndAnd => Some((BinaryOp::And, 2)),
            TokenKind::EqualEqual => Some((BinaryOp::Equal, 3)),
            TokenKind::BangEqual => Some((BinaryOp::NotEqual, 3)),
            TokenKind::Greater => Some((BinaryOp::Greater, 4)),
            TokenKind::GreaterEqual => Some((BinaryOp::GreaterEqual, 4)),
            TokenKind::Less => Some((BinaryOp::Less, 4)),
            TokenKind::LessEqual => Some((BinaryOp::LessEqual, 4)),
            TokenKind::Plus => Some((BinaryOp::Add, 5)),
            TokenKind::Minus => Some((BinaryOp::Sub, 5)),
            TokenKind::Star => Some((BinaryOp::Mul, 6)),
            TokenKind::Slash => Some((BinaryOp::Div, 6)),
            TokenKind::Percent => Some((BinaryOp::Mod, 6)),
            _ => None,
        }
    }

    fn expect_exact(&mut self, expected: TokenKind, label: &str) -> Result<Token, ParseError> {
        let token = self.advance_or_error(format!("expected {label}"))?;
        if discriminant_eq(&token.kind, &expected) {
            Ok(token)
        } else {
            Err(self.error_at(
                token.span,
                format!("expected {label} found {}", display_kind(&token.kind)),
            ))
        }
    }

    fn match_exact(&mut self, expected: TokenKind) -> bool {
        if self.check_exact(&expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn check_exact(&self, expected: &TokenKind) -> bool {
        self.current_kind()
            .is_some_and(|kind| discriminant_eq(kind, expected))
    }

    fn current_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.cursor).map(|token| &token.kind)
    }

    fn advance_or_error(&mut self, message: impl Into<String>) -> Result<Token, ParseError> {
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| self.error_here(message))?;
        self.cursor += 1;
        Ok(token)
    }

    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.cursor.saturating_sub(1))
            .map(|token| token.span)
            .unwrap_or_else(|| Span::new(0, 0))
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn parse_u64(&self, value: &str, span: Span) -> Result<u64, ParseError> {
        value
            .parse::<u64>()
            .map_err(|_| self.error_at(span, "integer literal exceeds u64 range"))
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        let span = self
            .tokens
            .get(self.cursor)
            .map(|token| token.span)
            .or_else(|| self.tokens.last().map(|token| token.span))
            .unwrap_or_else(|| Span::new(0, 0));
        self.error_at(span, message)
    }

    fn error_at(&self, span: Span, message: impl Into<String>) -> ParseError {
        let SourceLocation { file, line, column } = locate(self.source, self.file, span.start);
        ParseError {
            file,
            line,
            column,
            span,
            message: message.into(),
        }
    }
}

impl From<LexError> for ParseError {
    fn from(value: LexError) -> Self {
        Self {
            file: value.file,
            line: value.line,
            column: value.column,
            span: value.span,
            message: value.message,
        }
    }
}

fn is_value_keyword(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Covenant
            | TokenKind::CovenantId
            | TokenKind::ZkVerify
            | TokenKind::FinalityDepth
            | TokenKind::Sequencing
            | TokenKind::Multisig
            | TokenKind::InputBuiltin
            | TokenKind::OutputBuiltin
            | TokenKind::Block
    )
}

fn token_text(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Covenant => "covenant",
        TokenKind::CovenantId => "covenant_id",
        TokenKind::ZkVerify => "zk_verify",
        TokenKind::FinalityDepth => "finality_depth",
        TokenKind::Sequencing => "sequencing",
        TokenKind::Multisig => "multisig",
        TokenKind::InputBuiltin => "input",
        TokenKind::OutputBuiltin => "output",
        TokenKind::Block => "block",
        _ => "<token>",
    }
}

fn display_kind(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Identifier(value) => format!("identifier `{value}`"),
        TokenKind::Integer(value) => format!("integer `{value}`"),
        TokenKind::String(_) => "string".to_owned(),
        other => format!("{other:?}"),
    }
}

fn discriminant_eq(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaspascript_lexer::TypeName;

    #[test]
    fn extracts_finality_depth_from_params() {
        let program = parse(
            r#"
            contract Vault {
              params {
                owner: PublicKey,
                finality_depth: 10,
              }
              spend withdraw(sig: Signature) {
                require sig.verify(owner);
              }
            }
            "#,
        )
        .expect("parses");

        let contract = &program.contracts[0];
        assert_eq!(contract.finality_depth, Some(10));
        assert_eq!(contract.params.len(), 1);
        assert_eq!(contract.params[0].ty, TypeName::PublicKey);
    }

    #[test]
    fn parses_precedence_and_chained_calls() {
        let program = parse(
            r#"
            contract Escrow {
              params { owner: PublicKey }
              spend withdraw(sig: Signature) {
                require output(0).value >= input(0).value && sig.verify(owner);
              }
            }
            "#,
        )
        .expect("parses");

        assert_eq!(program.contracts[0].spends[0].body.len(), 1);
    }
}
