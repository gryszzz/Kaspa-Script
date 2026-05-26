use std::fmt;

use kaspascript_lexer::{lex, Keyword, LexError, Position, Span, Token, TokenKind, TypeName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub contracts: Vec<Contract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    pub name: Ident,
    pub params: Vec<Param>,
    pub has_params_block: bool,
    pub spends: Vec<Spend>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub value: ParamValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamValue {
    Type(TypeName),
    Integer(String),
    String(String),
    Bool(bool),
    Identifier(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spend {
    pub name: Ident,
    pub params: Vec<TypedParam>,
    pub requires: Vec<Require>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedParam {
    pub name: Ident,
    pub ty: TypeName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Require {
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Identifier(Ident),
    Integer {
        value: String,
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
    Member {
        object: Box<Expr>,
        field: Ident,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Identifier(ident) => ident.span,
            Expr::Integer { span, .. }
            | Expr::String { span, .. }
            | Expr::Bool { span, .. }
            | Expr::Array { span, .. }
            | Expr::Member { span, .. }
            | Expr::Call { span, .. }
            | Expr::Binary { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Equal,
    NotEqual,
    GreaterEqual,
    LessEqual,
    Greater,
    Less,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: Position,
}

impl ParseError {
    fn new(message: impl Into<String>, position: Position) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }

    fn from_lex_error(error: LexError) -> Self {
        Self {
            message: error.to_string(),
            position: error.position,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}, byte {}",
            self.message, self.position.line, self.position.column, self.position.offset
        )
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    let tokens = lex(source).map_err(ParseError::from_lex_error)?;
    Parser::new(tokens).parse_program()
}

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut contracts = Vec::new();

        while !self.is_at_end() {
            contracts.push(self.parse_contract()?);
        }

        Ok(Program { contracts })
    }

    fn parse_contract(&mut self) -> Result<Contract, ParseError> {
        let contract_token = self.expect_keyword(Keyword::Contract, "`contract`")?;
        let name = self.parse_name()?;
        self.expect_kind(TokenKind::LeftBrace, "`{`")?;

        let mut params = Vec::new();
        let mut has_params_block = false;
        let mut spends = Vec::new();

        while !self.check_kind(&TokenKind::RightBrace) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated contract block"));
            }

            match self.current_kind() {
                Some(TokenKind::Keyword(Keyword::Params)) => {
                    if has_params_block {
                        return Err(self.error_here("duplicate `params` block"));
                    }
                    has_params_block = true;
                    params = self.parse_params_block()?;
                }
                Some(TokenKind::Keyword(Keyword::Spend)) => {
                    spends.push(self.parse_spend()?);
                }
                _ => {
                    return Err(self.error_here("expected `params`, `spend`, or `}`"));
                }
            }
        }

        let end = self.expect_kind(TokenKind::RightBrace, "`}`")?;

        Ok(Contract {
            name,
            params,
            has_params_block,
            spends,
            span: Span::new(contract_token.span.start, end.span.end),
        })
    }

    fn parse_params_block(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect_keyword(Keyword::Params, "`params`")?;
        self.expect_kind(TokenKind::LeftBrace, "`{`")?;

        let mut params = Vec::new();
        while !self.check_kind(&TokenKind::RightBrace) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated params block"));
            }

            let name = self.parse_name()?;
            self.expect_kind(TokenKind::Colon, "`:`")?;
            let value = self.parse_param_value()?;
            let end = self.previous_end();
            let start = name.span.start;
            params.push(Param {
                name,
                value,
                span: Span::new(start, end),
            });

            if !self.match_kind(TokenKind::Comma) && !self.check_kind(&TokenKind::RightBrace) {
                return Err(self.error_here("expected `,` or `}` after parameter"));
            }
        }

        self.expect_kind(TokenKind::RightBrace, "`}`")?;
        Ok(params)
    }

    fn parse_param_value(&mut self) -> Result<ParamValue, ParseError> {
        let token = self.advance_or_error("expected parameter type or literal")?;
        match token.kind {
            TokenKind::Type(ty) => Ok(ParamValue::Type(ty)),
            TokenKind::Integer(value) => Ok(ParamValue::Integer(value)),
            TokenKind::String(value) => Ok(ParamValue::String(value)),
            TokenKind::Bool(value) => Ok(ParamValue::Bool(value)),
            TokenKind::Identifier(name) => Ok(ParamValue::Identifier(Ident {
                name,
                span: token.span,
            })),
            TokenKind::Keyword(keyword) if is_value_identifier_keyword(keyword) => {
                Ok(ParamValue::Identifier(Ident {
                    name: keyword_text(keyword).to_owned(),
                    span: token.span,
                }))
            }
            _ => Err(ParseError::new(
                "expected parameter type or literal",
                token.span.start,
            )),
        }
    }

    fn parse_spend(&mut self) -> Result<Spend, ParseError> {
        let spend_token = self.expect_keyword(Keyword::Spend, "`spend`")?;
        let name = self.parse_name()?;
        self.expect_kind(TokenKind::LeftParen, "`(`")?;
        let params = self.parse_typed_params()?;
        self.expect_kind(TokenKind::RightParen, "`)`")?;
        self.expect_kind(TokenKind::LeftBrace, "`{`")?;

        let mut requires = Vec::new();
        while !self.check_kind(&TokenKind::RightBrace) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated spend block"));
            }
            requires.push(self.parse_require()?);
        }

        let end = self.expect_kind(TokenKind::RightBrace, "`}`")?;
        Ok(Spend {
            name,
            params,
            requires,
            span: Span::new(spend_token.span.start, end.span.end),
        })
    }

    fn parse_typed_params(&mut self) -> Result<Vec<TypedParam>, ParseError> {
        let mut params = Vec::new();
        while !self.check_kind(&TokenKind::RightParen) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated spend parameter list"));
            }

            let name = self.parse_name()?;
            self.expect_kind(TokenKind::Colon, "`:`")?;
            let type_token = self.advance_or_error("expected type name")?;
            let ty = match type_token.kind {
                TokenKind::Type(ty) => ty,
                _ => return Err(ParseError::new("expected type name", type_token.span.start)),
            };
            let start = name.span.start;
            params.push(TypedParam {
                name,
                ty,
                span: Span::new(start, type_token.span.end),
            });

            if !self.match_kind(TokenKind::Comma) && !self.check_kind(&TokenKind::RightParen) {
                return Err(self.error_here("expected `,` or `)` after spend parameter"));
            }
        }

        Ok(params)
    }

    fn parse_require(&mut self) -> Result<Require, ParseError> {
        let require_token = self.expect_keyword(Keyword::Require, "`require`")?;
        let expr = self.parse_expression()?;
        let semicolon = self.expect_kind(TokenKind::Semicolon, "`;`")?;

        Ok(Require {
            expr,
            span: Span::new(require_token.span.start, semicolon.span.end),
        })
    }

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_postfix()?;

        while let Some(op) = self.match_binary_op() {
            let right = self.parse_postfix()?;
            let span = Span::new(expr.span().start, right.span().end);
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
                span,
            };
        }

        Ok(expr)
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_kind(TokenKind::Dot) {
                let field = self.parse_name()?;
                let span = Span::new(expr.span().start, field.span.end);
                expr = Expr::Member {
                    object: Box::new(expr),
                    field,
                    span,
                };
            } else if self.match_kind(TokenKind::LeftParen) {
                let args = self.parse_call_args()?;
                let right_paren = self.expect_kind(TokenKind::RightParen, "`)`")?;
                let span = Span::new(expr.span().start, right_paren.span.end);
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        while !self.check_kind(&TokenKind::RightParen) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated call argument list"));
            }

            args.push(self.parse_expression()?);

            if !self.match_kind(TokenKind::Comma) && !self.check_kind(&TokenKind::RightParen) {
                return Err(self.error_here("expected `,` or `)` after argument"));
            }
        }

        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.advance_or_error("expected expression")?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(Expr::Identifier(Ident {
                name,
                span: token.span,
            })),
            TokenKind::Keyword(keyword) if is_value_identifier_keyword(keyword) => {
                Ok(Expr::Identifier(Ident {
                    name: keyword_text(keyword).to_owned(),
                    span: token.span,
                }))
            }
            TokenKind::Integer(value) => Ok(Expr::Integer {
                value,
                span: token.span,
            }),
            TokenKind::String(value) => Ok(Expr::String {
                value,
                span: token.span,
            }),
            TokenKind::Bool(value) => Ok(Expr::Bool {
                value,
                span: token.span,
            }),
            TokenKind::LeftParen => {
                let expr = self.parse_expression()?;
                self.expect_kind(TokenKind::RightParen, "`)`")?;
                Ok(expr)
            }
            TokenKind::LeftBracket => self.finish_array(token.span.start),
            _ => Err(ParseError::new("expected expression", token.span.start)),
        }
    }

    fn finish_array(&mut self, start: Position) -> Result<Expr, ParseError> {
        let mut elements = Vec::new();
        while !self.check_kind(&TokenKind::RightBracket) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated array literal"));
            }

            elements.push(self.parse_expression()?);

            if !self.match_kind(TokenKind::Comma) && !self.check_kind(&TokenKind::RightBracket) {
                return Err(self.error_here("expected `,` or `]` after array element"));
            }
        }

        let end = self.expect_kind(TokenKind::RightBracket, "`]`")?;
        Ok(Expr::Array {
            elements,
            span: Span::new(start, end.span.end),
        })
    }

    fn parse_name(&mut self) -> Result<Ident, ParseError> {
        let token = self.advance_or_error("expected identifier")?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(Ident {
                name,
                span: token.span,
            }),
            TokenKind::Keyword(keyword) if is_value_identifier_keyword(keyword) => Ok(Ident {
                name: keyword_text(keyword).to_owned(),
                span: token.span,
            }),
            _ => Err(ParseError::new("expected identifier", token.span.start)),
        }
    }

    fn match_binary_op(&mut self) -> Option<BinaryOp> {
        let op = match self.current_kind()? {
            TokenKind::EqualEqual => BinaryOp::Equal,
            TokenKind::BangEqual => BinaryOp::NotEqual,
            TokenKind::GreaterEqual => BinaryOp::GreaterEqual,
            TokenKind::LessEqual => BinaryOp::LessEqual,
            TokenKind::Greater => BinaryOp::Greater,
            TokenKind::Less => BinaryOp::Less,
            _ => return None,
        };
        self.cursor += 1;
        Some(op)
    }

    fn expect_keyword(&mut self, keyword: Keyword, label: &str) -> Result<Token, ParseError> {
        let token = self.advance_or_error(format!("expected {label}"))?;
        match token.kind {
            TokenKind::Keyword(actual) if actual == keyword => Ok(token),
            _ => Err(ParseError::new(
                format!("expected {label}"),
                token.span.start,
            )),
        }
    }

    fn expect_kind(&mut self, kind: TokenKind, label: &str) -> Result<Token, ParseError> {
        let token = self.advance_or_error(format!("expected {label}"))?;
        if token.kind == kind {
            Ok(token)
        } else {
            Err(ParseError::new(
                format!("expected {label}"),
                token.span.start,
            ))
        }
    }

    fn match_kind(&mut self, kind: TokenKind) -> bool {
        if self.check_kind(&kind) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn check_kind(&self, kind: &TokenKind) -> bool {
        self.current_kind().is_some_and(|current| current == kind)
    }

    fn current_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.cursor).map(|token| &token.kind)
    }

    fn advance_or_error(&mut self, message: impl Into<String>) -> Result<Token, ParseError> {
        if self.is_at_end() {
            return Err(ParseError::new(message, self.current_position()));
        }

        let token = self.tokens[self.cursor].clone();
        self.cursor += 1;
        Ok(token)
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn current_position(&self) -> Position {
        self.tokens
            .get(self.cursor)
            .map(|token| token.span.start)
            .or_else(|| self.tokens.last().map(|token| token.span.end))
            .unwrap_or_else(|| Position::new(1, 1, 0))
    }

    fn previous_end(&self) -> Position {
        self.tokens
            .get(self.cursor.saturating_sub(1))
            .map(|token| token.span.end)
            .unwrap_or_else(|| Position::new(1, 1, 0))
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(message, self.current_position())
    }
}

fn is_value_identifier_keyword(keyword: Keyword) -> bool {
    matches!(
        keyword,
        Keyword::Covenant
            | Keyword::ZkVerify
            | Keyword::FinalityDepth
            | Keyword::CovenantId
            | Keyword::Sequencing
    )
}

fn keyword_text(keyword: Keyword) -> &'static str {
    match keyword {
        Keyword::Contract => "contract",
        Keyword::Params => "params",
        Keyword::Spend => "spend",
        Keyword::Require => "require",
        Keyword::Covenant => "covenant",
        Keyword::ZkVerify => "zk_verify",
        Keyword::FinalityDepth => "finality_depth",
        Keyword::CovenantId => "covenant_id",
        Keyword::Sequencing => "sequencing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_production_vault_contract() {
        let source = include_str!("../../../contracts/production/DAGSafeVault.ks");
        let program = parse(source).expect("production vault must parse");

        assert_eq!(program.contracts.len(), 1);
        let contract = &program.contracts[0];
        assert_eq!(contract.name.name, "DAGSafeVault");
        assert!(contract.has_params_block);
        assert_eq!(contract.params.len(), 5);
        assert_eq!(contract.spends.len(), 3);
        assert!(contract
            .params
            .iter()
            .any(|param| param.name.name == "finality_depth"
                && param.value == ParamValue::Integer("10".to_owned())));
    }

    #[test]
    fn parses_calls_members_arrays_and_comparisons() {
        let source = r#"
            contract Escrow {
              params {
                buyer: PublicKey,
                seller: PublicKey,
                arbiter: PublicKey,
              }

              spend release(sig_a: Signature, sig_b: Signature) {
                require multisig(2, [buyer, seller, arbiter], [sig_a, sig_b]);
                require output(0).value >= input(0).value;
              }
            }
        "#;

        let program = parse(source).expect("escrow fragment must parse");
        let spend = &program.contracts[0].spends[0];

        assert_eq!(spend.params.len(), 2);
        assert_eq!(spend.requires.len(), 2);
        assert!(matches!(spend.requires[1].expr, Expr::Binary { .. }));
    }

    #[test]
    fn accepts_toccata_keywords_as_value_identifiers() {
        let source = r#"
            contract ZKGate {
              params {
                proof: ZKProof,
                finality_depth: 8,
              }

              spend settle(sig: Signature) {
                require zk_verify(proof);
                require covenant_id == covenant.id;
                require sequencing.depth >= finality_depth;
              }
            }
        "#;

        let program = parse(source).expect("toccata identifiers must parse");
        assert_eq!(program.contracts[0].spends[0].requires.len(), 3);
    }

    #[test]
    fn reports_position_for_missing_semicolon() {
        let source = r#"
contract Broken {
  params { owner: PublicKey }
  spend withdraw(sig: Signature) {
    require sig.verify(owner)
  }
}
"#;

        let error = parse(source).expect_err("missing semicolon must fail");
        assert_eq!(error.position.line, 6);
        assert_eq!(error.position.column, 3);
    }
}
