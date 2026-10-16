pub mod token;

use crate::diag::{Diagnostic, Result};
use crate::span::Span;
pub use token::{InterpolatedPart, Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    file_id: usize,
    cursor: usize,
    paren_depth: usize,
    brace_depth: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str, file_id: usize) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            file_id,
            cursor: 0,
            paren_depth: 0,
            brace_depth: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens: Vec<Token> = Vec::new();

        while self.cursor < self.bytes.len() {
            let start = self.cursor;
            let ch = self.peek_char();

            // Handle comments
            if ch == '/' && self.peek_char_offset(1) == '/' {
                self.skip_line_comment();
                continue;
            }
            if ch == '/' && self.peek_char_offset(1) == '*' {
                self.skip_block_comment()?;
                continue;
            }

            // Handle newlines
            if ch == '\n' {
                self.cursor += 1;
                if self.paren_depth == 0 {
                    if let Some(last) = tokens.last() {
                        if last.can_end_statement() {
                            let span = Span::new(start, self.cursor, self.file_id);
                            tokens.push(Token::new(TokenKind::Newline, span));
                        }
                    }
                }
                continue;
            }

            // Handle whitespace
            if ch.is_whitespace() {
                self.cursor += 1;
                continue;
            }

            // Two-character and multi-character operators
            if ch == ':' && self.peek_char_offset(1) == '=' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::Walrus, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '-' && self.peek_char_offset(1) == '>' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::Arrow, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '=' && self.peek_char_offset(1) == '=' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::EqEq, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '!' && self.peek_char_offset(1) == '=' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::NotEq, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '<' && self.peek_char_offset(1) == '=' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::LtEq, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '>' && self.peek_char_offset(1) == '=' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::GtEq, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '&' && self.peek_char_offset(1) == '&' {
                self.cursor += 2;
                tokens.push(Token::new(TokenKind::AndAnd, Span::new(start, self.cursor, self.file_id)));
                continue;
            }
            if ch == '|' {
                if self.peek_char_offset(1) == '|' {
                    self.cursor += 2;
                    tokens.push(Token::new(TokenKind::OrOr, Span::new(start, self.cursor, self.file_id)));
                } else {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Pipe, Span::new(start, self.cursor, self.file_id)));
                }
                continue;
            }

            // Single-character delimiters and operators
            match ch {
                '(' => {
                    self.cursor += 1;
                    self.paren_depth += 1;
                    tokens.push(Token::new(TokenKind::LParen, Span::new(start, self.cursor, self.file_id)));
                }
                ')' => {
                    self.cursor += 1;
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                    tokens.push(Token::new(TokenKind::RParen, Span::new(start, self.cursor, self.file_id)));
                }
                '{' => {
                    self.cursor += 1;
                    self.brace_depth += 1;
                    tokens.push(Token::new(TokenKind::LBrace, Span::new(start, self.cursor, self.file_id)));
                }
                '}' => {
                    self.cursor += 1;
                    self.brace_depth = self.brace_depth.saturating_sub(1);
                    tokens.push(Token::new(TokenKind::RBrace, Span::new(start, self.cursor, self.file_id)));
                }
                '[' => {
                    self.cursor += 1;
                    self.paren_depth += 1;
                    tokens.push(Token::new(TokenKind::LBracket, Span::new(start, self.cursor, self.file_id)));
                }
                ']' => {
                    self.cursor += 1;
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                    tokens.push(Token::new(TokenKind::RBracket, Span::new(start, self.cursor, self.file_id)));
                }
                ',' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Comma, Span::new(start, self.cursor, self.file_id)));
                }
                ':' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Colon, Span::new(start, self.cursor, self.file_id)));
                }
                ';' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Semicolon, Span::new(start, self.cursor, self.file_id)));
                }
                '.' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Dot, Span::new(start, self.cursor, self.file_id)));
                }
                '?' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Question, Span::new(start, self.cursor, self.file_id)));
                }
                '!' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Exclamation, Span::new(start, self.cursor, self.file_id)));
                }
                '+' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Plus, Span::new(start, self.cursor, self.file_id)));
                }
                '-' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Minus, Span::new(start, self.cursor, self.file_id)));
                }
                '*' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Star, Span::new(start, self.cursor, self.file_id)));
                }
                '/' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Slash, Span::new(start, self.cursor, self.file_id)));
                }
                '%' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Percent, Span::new(start, self.cursor, self.file_id)));
                }
                '=' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Eq, Span::new(start, self.cursor, self.file_id)));
                }
                '<' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Lt, Span::new(start, self.cursor, self.file_id)));
                }
                '>' => {
                    self.cursor += 1;
                    tokens.push(Token::new(TokenKind::Gt, Span::new(start, self.cursor, self.file_id)));
                }
                '"' => {
                    let tok = self.lex_string(start)?;
                    tokens.push(tok);
                }
                _ if ch.is_ascii_digit() => {
                    let tok = self.lex_number(start)?;
                    tokens.push(tok);
                }
                _ if ch.is_alphabetic() || ch == '_' => {
                    let tok = self.lex_identifier_or_keyword(start);
                    tokens.push(tok);
                }
                _ => {
                    return Err(Diagnostic::error(format!("Karakter tidak dikenal: '{}'", ch))
                        .with_span(Span::new(start, start + 1, self.file_id)));
                }
            }
        }

        let eof_span = Span::new(self.cursor, self.cursor, self.file_id);
        tokens.push(Token::new(TokenKind::Eof, eof_span));
        Ok(tokens)
    }

    fn peek_char(&self) -> char {
        if self.cursor >= self.bytes.len() {
            '\0'
        } else {
            self.bytes[self.cursor] as char
        }
    }

    fn peek_char_offset(&self, offset: usize) -> char {
        let idx = self.cursor + offset;
        if idx >= self.bytes.len() {
            '\0'
        } else {
            self.bytes[idx] as char
        }
    }

    fn skip_line_comment(&mut self) {
        while self.cursor < self.bytes.len() && self.bytes[self.cursor] != b'\n' {
            self.cursor += 1;
        }
    }

    fn skip_block_comment(&mut self) -> Result<()> {
        let start = self.cursor;
        self.cursor += 2; // skip /*
        let mut depth = 1;
        while self.cursor < self.bytes.len() && depth > 0 {
            if self.bytes[self.cursor] == b'/' && self.peek_char_offset(1) == '*' {
                depth += 1;
                self.cursor += 2;
            } else if self.bytes[self.cursor] == b'*' && self.peek_char_offset(1) == '/' {
                depth -= 1;
                self.cursor += 2;
            } else {
                self.cursor += 1;
            }
        }
        if depth > 0 {
            return Err(Diagnostic::error("Komentar blok tidak ditutup")
                .with_span(Span::new(start, self.cursor, self.file_id)));
        }
        Ok(())
    }

    fn lex_number(&mut self, start: usize) -> Result<Token> {
        while self.cursor < self.bytes.len() && (self.bytes[self.cursor].is_ascii_digit() || self.bytes[self.cursor] == b'_') {
            self.cursor += 1;
        }

        let is_float = if self.peek_char() == '.' && self.peek_char_offset(1).is_ascii_digit() {
            self.cursor += 1; // skip '.'
            while self.cursor < self.bytes.len() && (self.bytes[self.cursor].is_ascii_digit() || self.bytes[self.cursor] == b'_') {
                self.cursor += 1;
            }
            true
        } else {
            false
        };

        let raw = &self.src[start..self.cursor];
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();

        let span = Span::new(start, self.cursor, self.file_id);
        if is_float {
            match cleaned.parse::<f64>() {
                Ok(val) => Ok(Token::new(TokenKind::FloatLit(val), span)),
                Err(e) => Err(Diagnostic::error(format!("Literal pecahan tidak valid: {}", e)).with_span(span)),
            }
        } else {
            match cleaned.parse::<u64>() {
                Ok(val) => Ok(Token::new(TokenKind::IntLit(val), span)),
                Err(e) => Err(Diagnostic::error(format!("Literal bilangan bulat tidak valid: {}", e)).with_span(span)),
            }
        }
    }

    fn lex_identifier_or_keyword(&mut self, start: usize) -> Token {
        while self.cursor < self.bytes.len() {
            let ch = self.bytes[self.cursor] as char;
            if ch.is_alphanumeric() || ch == '_' {
                self.cursor += 1;
            } else {
                break;
            }
        }

        let word = &self.src[start..self.cursor];
        let span = Span::new(start, self.cursor, self.file_id);

        let kind = match word {
            "struktur" => TokenKind::Struktur,
            "sifat" => TokenKind::Sifat,
            "terapkan" => TokenKind::Terapkan,
            "untuk" => TokenKind::Untuk,
            "fungsi" => TokenKind::Fungsi,
            "jika" => TokenKind::Jika,
            "selain_itu" => TokenKind::SelainItu,
            "selama" => TokenKind::Selama,
            "kembali" => TokenKind::Kembali,
            "gagal" => TokenKind::Gagal,
            "tangkap" => TokenKind::Tangkap,
            "jalan_paralel" => TokenKind::JalanParalel,
            "tunggu" => TokenKind::Tunggu,
            "comptime" => TokenKind::Comptime,
            "ini" => TokenKind::Ini,
            "di" => TokenKind::Di,
            "dalam" => TokenKind::Dalam,
            "impor" => TokenKind::Impor,
            "uji" => TokenKind::Uji,
            "benar" => TokenKind::Benar,
            "salah" => TokenKind::Salah,
            _ => TokenKind::Ident(word.to_string()),
        };

        Token::new(kind, span)
    }

    fn lex_string(&mut self, start: usize) -> Result<Token> {
        self.cursor += 1; // skip opening "
        let mut parts = Vec::new();
        let mut cur_literal = String::new();
        let mut has_interpolation = false;

        while self.cursor < self.bytes.len() {
            let ch = self.bytes[self.cursor] as char;

            if ch == '"' {
                self.cursor += 1;
                let span = Span::new(start, self.cursor, self.file_id);
                if has_interpolation {
                    if !cur_literal.is_empty() {
                        parts.push(InterpolatedPart::Literal(cur_literal));
                    }
                    return Ok(Token::new(TokenKind::InterpolatedString(parts), span));
                } else {
                    return Ok(Token::new(TokenKind::StringLit(cur_literal), span));
                }
            } else if ch == '\\' {
                self.cursor += 1;
                if self.cursor >= self.bytes.len() {
                    break;
                }
                let esc = self.bytes[self.cursor] as char;
                self.cursor += 1;
                match esc {
                    'n' => cur_literal.push('\n'),
                    'r' => cur_literal.push('\r'),
                    't' => cur_literal.push('\t'),
                    '\\' => cur_literal.push('\\'),
                    '"' => cur_literal.push('"'),
                    '{' => cur_literal.push('{'),
                    '}' => cur_literal.push('}'),
                    _ => {
                        cur_literal.push('\\');
                        cur_literal.push(esc);
                    }
                }
            } else if ch == '{' {
                self.cursor += 1; // skip '{'
                let mut expr_buf = String::new();
                let mut inner_braces = 0;
                let mut in_inner_str = false;
                let mut closed = false;
                while self.cursor < self.bytes.len() {
                    let ic = self.bytes[self.cursor] as char;
                    if ic == '\\' {
                        self.cursor += 1;
                        if self.cursor < self.bytes.len() {
                            let esc = self.bytes[self.cursor] as char;
                            self.cursor += 1;
                            if esc == '"' {
                                in_inner_str = !in_inner_str;
                                expr_buf.push('"');
                            } else if esc == '\\' {
                                expr_buf.push('\\');
                            } else {
                                expr_buf.push('\\');
                                expr_buf.push(esc);
                            }
                        }
                    } else if ic == '"' {
                        in_inner_str = !in_inner_str;
                        expr_buf.push(ic);
                        self.cursor += 1;
                    } else if ic == '{' && !in_inner_str {
                        inner_braces += 1;
                        expr_buf.push(ic);
                        self.cursor += 1;
                    } else if ic == '}' && !in_inner_str {
                        if inner_braces == 0 {
                            self.cursor += 1;
                            closed = true;
                            break;
                        } else {
                            inner_braces -= 1;
                            expr_buf.push(ic);
                            self.cursor += 1;
                        }
                    } else {
                        expr_buf.push(ic);
                        self.cursor += 1;
                    }
                }

                if closed && expr_buf.trim().is_empty() {
                    // Kurung kurawal kosong seperti "{}" dianggap teks literal (misal objek JSON kosong)
                    cur_literal.push('{');
                    cur_literal.push_str(&expr_buf);
                    cur_literal.push('}');
                } else if closed {
                    has_interpolation = true;
                    if !cur_literal.is_empty() {
                        parts.push(InterpolatedPart::Literal(cur_literal));
                        cur_literal = String::new();
                    }
                    parts.push(InterpolatedPart::Expression(expr_buf.trim().to_string()));
                } else {
                    // Kurung kurawal tidak ditutup sebelum akhir string
                    cur_literal.push('{');
                    cur_literal.push_str(&expr_buf);
                }
            } else {
                cur_literal.push(ch);
                self.cursor += 1;
            }
        }

        Err(Diagnostic::error("Literal teks tidak ditutup tanda kutip ganda (\")")
            .with_span(Span::new(start, self.cursor, self.file_id)))
    }
}
