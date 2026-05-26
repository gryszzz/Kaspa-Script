use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Contract,
    Params,
    Spend,
    Require,
    Covenant,
    ZkVerify,
    FinalityDepth,
    CovenantId,
    Sequencing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeName {
    PublicKey,
    Signature,
    Hash,
    BlockHeight,
    Amount,
    Bool,
    Bytes,
    CovenantID,
    UTXO,
    Output,
    Input,
    ZKProof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Keyword(Keyword),
    Type(TypeName),
    Identifier(String),
    Integer(String),
    String(String),
    Bool(bool),
    Equal,
    EqualEqual,
    BangEqual,
    GreaterEqual,
    LessEqual,
    Greater,
    Less,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    fn new(kind: TokenKind, start: Position, end: Position) -> Self {
        Self {
            kind,
            span: Span::new(start, end),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    UnknownCharacter(char),
    UnterminatedString,
    UnterminatedBlockComment,
    InvalidEscape(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub position: Position,
}

impl LexError {
    fn new(kind: LexErrorKind, position: Position) -> Self {
        Self { kind, position }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            LexErrorKind::UnknownCharacter(ch) => write!(
                f,
                "unknown character {:?} at line {}, column {}, byte {}",
                ch, self.position.line, self.position.column, self.position.offset
            ),
            LexErrorKind::UnterminatedString => write!(
                f,
                "unterminated string at line {}, column {}, byte {}",
                self.position.line, self.position.column, self.position.offset
            ),
            LexErrorKind::UnterminatedBlockComment => write!(
                f,
                "unterminated block comment at line {}, column {}, byte {}",
                self.position.line, self.position.column, self.position.offset
            ),
            LexErrorKind::InvalidEscape(ch) => write!(
                f,
                "invalid escape sequence \\{} at line {}, column {}, byte {}",
                ch, self.position.line, self.position.column, self.position.offset
            ),
        }
    }
}

impl std::error::Error for LexError {}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).lex()
}

pub struct Lexer<'source> {
    source: &'source str,
    offset: usize,
    line: usize,
    column: usize,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            source,
            offset: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn lex(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while !self.is_at_end() {
            self.skip_whitespace_and_comments()?;
            if self.is_at_end() {
                break;
            }

            tokens.push(self.next_token()?);
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        let start = self.position();
        let ch = self
            .peek_char()
            .expect("next_token is only called when not at end");

        if is_identifier_start(ch) {
            return Ok(self.identifier_or_reserved());
        }

        if ch.is_ascii_digit() {
            return Ok(self.integer());
        }

        if ch == '"' {
            return self.string();
        }

        match ch {
            '=' => {
                self.bump_char();
                if self.consume_if('=') {
                    Ok(Token::new(TokenKind::EqualEqual, start, self.position()))
                } else {
                    Ok(Token::new(TokenKind::Equal, start, self.position()))
                }
            }
            '!' => {
                self.bump_char();
                if self.consume_if('=') {
                    Ok(Token::new(TokenKind::BangEqual, start, self.position()))
                } else {
                    Err(LexError::new(LexErrorKind::UnknownCharacter('!'), start))
                }
            }
            '>' => {
                self.bump_char();
                if self.consume_if('=') {
                    Ok(Token::new(TokenKind::GreaterEqual, start, self.position()))
                } else {
                    Ok(Token::new(TokenKind::Greater, start, self.position()))
                }
            }
            '<' => {
                self.bump_char();
                if self.consume_if('=') {
                    Ok(Token::new(TokenKind::LessEqual, start, self.position()))
                } else {
                    Ok(Token::new(TokenKind::Less, start, self.position()))
                }
            }
            '-' => {
                self.bump_char();
                if self.consume_if('>') {
                    Ok(Token::new(TokenKind::Arrow, start, self.position()))
                } else {
                    Err(LexError::new(LexErrorKind::UnknownCharacter('-'), start))
                }
            }
            '{' => self.single_char(TokenKind::LeftBrace),
            '}' => self.single_char(TokenKind::RightBrace),
            '(' => self.single_char(TokenKind::LeftParen),
            ')' => self.single_char(TokenKind::RightParen),
            '[' => self.single_char(TokenKind::LeftBracket),
            ']' => self.single_char(TokenKind::RightBracket),
            ',' => self.single_char(TokenKind::Comma),
            ':' => self.single_char(TokenKind::Colon),
            ';' => self.single_char(TokenKind::Semicolon),
            '.' => self.single_char(TokenKind::Dot),
            other => Err(LexError::new(LexErrorKind::UnknownCharacter(other), start)),
        }
    }

    fn single_char(&mut self, kind: TokenKind) -> Result<Token, LexError> {
        let start = self.position();
        self.bump_char();
        Ok(Token::new(kind, start, self.position()))
    }

    fn identifier_or_reserved(&mut self) -> Token {
        let start = self.position();
        let start_offset = self.offset;

        self.bump_char();
        while self.peek_char().is_some_and(is_identifier_continue) {
            self.bump_char();
        }

        let text = &self.source[start_offset..self.offset];
        let kind = match_keyword(text)
            .map(TokenKind::Keyword)
            .or_else(|| match_type(text).map(TokenKind::Type))
            .unwrap_or_else(|| match text {
                "true" => TokenKind::Bool(true),
                "false" => TokenKind::Bool(false),
                _ => TokenKind::Identifier(text.to_owned()),
            });

        Token::new(kind, start, self.position())
    }

    fn integer(&mut self) -> Token {
        let start = self.position();
        let start_offset = self.offset;

        self.bump_char();
        while self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump_char();
        }

        Token::new(
            TokenKind::Integer(self.source[start_offset..self.offset].to_owned()),
            start,
            self.position(),
        )
    }

    fn string(&mut self) -> Result<Token, LexError> {
        let start = self.position();
        self.bump_char();

        let mut value = String::new();

        loop {
            let ch = match self.peek_char() {
                Some(ch) => ch,
                None => return Err(LexError::new(LexErrorKind::UnterminatedString, start)),
            };

            match ch {
                '"' => {
                    self.bump_char();
                    return Ok(Token::new(TokenKind::String(value), start, self.position()));
                }
                '\n' | '\r' => {
                    return Err(LexError::new(LexErrorKind::UnterminatedString, start));
                }
                '\\' => {
                    let escape_position = self.position();
                    self.bump_char();
                    let escaped = match self.peek_char() {
                        Some(ch) => ch,
                        None => {
                            return Err(LexError::new(LexErrorKind::UnterminatedString, start));
                        }
                    };

                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '0' => value.push('\0'),
                        '\n' | '\r' => {
                            return Err(LexError::new(LexErrorKind::UnterminatedString, start));
                        }
                        other => {
                            return Err(LexError::new(
                                LexErrorKind::InvalidEscape(other),
                                escape_position,
                            ));
                        }
                    }
                    self.bump_char();
                }
                other => {
                    value.push(other);
                    self.bump_char();
                }
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek_char() {
                Some(ch) if ch.is_whitespace() => {
                    self.bump_char();
                }
                Some('/') if self.peek_next_char() == Some('/') => {
                    self.bump_char();
                    self.bump_char();
                    while let Some(ch) = self.peek_char() {
                        if ch == '\n' || ch == '\r' {
                            break;
                        }
                        self.bump_char();
                    }
                }
                Some('/') if self.peek_next_char() == Some('*') => {
                    self.skip_block_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LexError> {
        let start = self.position();
        self.bump_char();
        self.bump_char();

        loop {
            match self.peek_char() {
                Some('*') if self.peek_next_char() == Some('/') => {
                    self.bump_char();
                    self.bump_char();
                    return Ok(());
                }
                Some(_) => {
                    self.bump_char();
                }
                None => {
                    return Err(LexError::new(LexErrorKind::UnterminatedBlockComment, start));
                }
            }
        }
    }

    fn consume_if(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.bump_char();
            true
        } else {
            false
        }
    }

    fn position(&self) -> Position {
        Position::new(self.line, self.column, self.offset)
    }

    fn is_at_end(&self) -> bool {
        self.offset >= self.source.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.source[self.offset..].chars();
        chars.next()?;
        chars.next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset += ch.len_utf8();

        if ch == '\r' {
            if self.peek_char() == Some('\n') {
                self.offset += '\n'.len_utf8();
            }
            self.line += 1;
            self.column = 1;
            return Some('\n');
        }

        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        Some(ch)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn match_keyword(text: &str) -> Option<Keyword> {
    match text {
        "contract" => Some(Keyword::Contract),
        "params" => Some(Keyword::Params),
        "spend" => Some(Keyword::Spend),
        "require" => Some(Keyword::Require),
        "covenant" => Some(Keyword::Covenant),
        "zk_verify" => Some(Keyword::ZkVerify),
        "finality_depth" => Some(Keyword::FinalityDepth),
        "covenant_id" => Some(Keyword::CovenantId),
        "sequencing" => Some(Keyword::Sequencing),
        _ => None,
    }
}

fn match_type(text: &str) -> Option<TypeName> {
    match text {
        "PublicKey" => Some(TypeName::PublicKey),
        "Signature" => Some(TypeName::Signature),
        "Hash" => Some(TypeName::Hash),
        "BlockHeight" => Some(TypeName::BlockHeight),
        "Amount" => Some(TypeName::Amount),
        "Bool" => Some(TypeName::Bool),
        "Bytes" => Some(TypeName::Bytes),
        "CovenantID" => Some(TypeName::CovenantID),
        "UTXO" => Some(TypeName::UTXO),
        "Output" => Some(TypeName::Output),
        "Input" => Some(TypeName::Input),
        "ZKProof" => Some(TypeName::ZKProof),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source)
            .expect("source should lex")
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn all_keywords() {
        assert_eq!(
            kinds(
                "contract params spend require covenant zk_verify finality_depth covenant_id sequencing"
            ),
            vec![
                TokenKind::Keyword(Keyword::Contract),
                TokenKind::Keyword(Keyword::Params),
                TokenKind::Keyword(Keyword::Spend),
                TokenKind::Keyword(Keyword::Require),
                TokenKind::Keyword(Keyword::Covenant),
                TokenKind::Keyword(Keyword::ZkVerify),
                TokenKind::Keyword(Keyword::FinalityDepth),
                TokenKind::Keyword(Keyword::CovenantId),
                TokenKind::Keyword(Keyword::Sequencing),
            ]
        );
    }

    #[test]
    fn all_types() {
        assert_eq!(
            kinds("PublicKey Signature Hash BlockHeight Amount Bool Bytes CovenantID UTXO Output Input ZKProof"),
            vec![
                TokenKind::Type(TypeName::PublicKey),
                TokenKind::Type(TypeName::Signature),
                TokenKind::Type(TypeName::Hash),
                TokenKind::Type(TypeName::BlockHeight),
                TokenKind::Type(TypeName::Amount),
                TokenKind::Type(TypeName::Bool),
                TokenKind::Type(TypeName::Bytes),
                TokenKind::Type(TypeName::CovenantID),
                TokenKind::Type(TypeName::UTXO),
                TokenKind::Type(TypeName::Output),
                TokenKind::Type(TypeName::Input),
                TokenKind::Type(TypeName::ZKProof),
            ]
        );
    }

    #[test]
    fn identifiers_vs_keywords() {
        assert_eq!(
            kinds("contract contract_id params_ zk_verify2 PublicKeys true false"),
            vec![
                TokenKind::Keyword(Keyword::Contract),
                TokenKind::Identifier("contract_id".to_owned()),
                TokenKind::Identifier("params_".to_owned()),
                TokenKind::Identifier("zk_verify2".to_owned()),
                TokenKind::Identifier("PublicKeys".to_owned()),
                TokenKind::Bool(true),
                TokenKind::Bool(false),
            ]
        );
    }

    #[test]
    fn integer_literals() {
        assert_eq!(
            kinds("0 7 10 184467440737095516160000"),
            vec![
                TokenKind::Integer("0".to_owned()),
                TokenKind::Integer("7".to_owned()),
                TokenKind::Integer("10".to_owned()),
                TokenKind::Integer("184467440737095516160000".to_owned()),
            ]
        );
    }

    #[test]
    fn string_literals() {
        assert_eq!(
            kinds(r#""kaspa" "escaped \"quote\"" "line\nfeed" "tab\tend""#),
            vec![
                TokenKind::String("kaspa".to_owned()),
                TokenKind::String("escaped \"quote\"".to_owned()),
                TokenKind::String("line\nfeed".to_owned()),
                TokenKind::String("tab\tend".to_owned()),
            ]
        );
    }

    #[test]
    fn operators_and_delimiters() {
        assert_eq!(
            kinds("= == != >= <= > < -> { } ( ) [ ] , : ; ."),
            vec![
                TokenKind::Equal,
                TokenKind::EqualEqual,
                TokenKind::BangEqual,
                TokenKind::GreaterEqual,
                TokenKind::LessEqual,
                TokenKind::Greater,
                TokenKind::Less,
                TokenKind::Arrow,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBracket,
                TokenKind::RightBracket,
                TokenKind::Comma,
                TokenKind::Colon,
                TokenKind::Semicolon,
                TokenKind::Dot,
            ]
        );
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(
            kinds(
                r#"
                contract // ignored line
                /* ignored
                   block */ spend
                "#
            ),
            vec![
                TokenKind::Keyword(Keyword::Contract),
                TokenKind::Keyword(Keyword::Spend),
            ]
        );
    }

    #[test]
    fn source_positions() {
        let tokens = lex("contract Vault {\n  require output(0).value >= input(0).value;\n}")
            .expect("source should lex");

        assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::Contract));
        assert_eq!(tokens[0].span.start, Position::new(1, 1, 0));
        assert_eq!(tokens[0].span.end, Position::new(1, 9, 8));

        let require = tokens
            .iter()
            .find(|token| token.kind == TokenKind::Keyword(Keyword::Require))
            .expect("require token");
        assert_eq!(require.span.start, Position::new(2, 3, 19));
        assert_eq!(require.span.end, Position::new(2, 10, 26));

        let ge = tokens
            .iter()
            .find(|token| token.kind == TokenKind::GreaterEqual)
            .expect(">= token");
        assert_eq!(ge.span.start, Position::new(2, 27, 43));
        assert_eq!(ge.span.end, Position::new(2, 29, 45));
    }

    #[test]
    fn invalid_token_error() {
        let error = lex("contract @").expect_err("invalid character should error");
        assert_eq!(error.kind, LexErrorKind::UnknownCharacter('@'));
        assert_eq!(error.position, Position::new(1, 10, 9));
    }

    #[test]
    fn unterminated_string_error() {
        let error = lex("params {\n  name: \"unterminated").expect_err("unterminated string");
        assert_eq!(error.kind, LexErrorKind::UnterminatedString);
        assert_eq!(error.position, Position::new(2, 9, 17));
    }

    #[test]
    fn unterminated_block_comment_error() {
        let error = lex("contract /* never closed").expect_err("unterminated block comment");
        assert_eq!(error.kind, LexErrorKind::UnterminatedBlockComment);
        assert_eq!(error.position, Position::new(1, 10, 9));
    }
}
