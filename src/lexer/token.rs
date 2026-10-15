use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Struktur,      // struktur
    Sifat,         // sifat
    Terapkan,      // terapkan
    Untuk,         // untuk
    Fungsi,        // fungsi
    Jika,          // jika
    SelainItu,     // selain_itu
    Selama,        // selama
    Kembali,       // kembali
    Gagal,         // gagal
    Tangkap,       // tangkap
    JalanParalel,  // jalan_paralel
    Tunggu,        // tunggu
    Comptime,      // comptime
    Ini,           // ini
    Di,            // di
    Dalam,         // dalam
    Impor,         // impor
    Benar,         // benar
    Salah,         // salah

    // Literals
    IntLit(u64),
    FloatLit(f64),
    StringLit(String),
    InterpolatedString(Vec<InterpolatedPart>),

    // Identifiers
    Ident(String),

    // Operators and Delimiters
    Walrus,        // :=
    Arrow,         // ->
    Exclamation,   // !
    Question,      // ?
    Dot,           // .
    Comma,         // ,
    Colon,         // :
    Semicolon,     // ;
    LParen,        // (
    RParen,        // )
    LBrace,        // {
    RBrace,        // }
    LBracket,      // [
    RBracket,      // ]
    Plus,          // +
    Minus,         // -
    Star,          // *
    Slash,         // /
    Percent,       // %
    Eq,            // =
    EqEq,          // ==
    NotEq,         // !=
    Lt,            // <
    LtEq,          // <=
    Gt,            // >
    GtEq,          // >=
    AndAnd,        // &&
    OrOr,          // ||

    // Statement separator inserted by newline
    Newline,

    // End of file
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpolatedPart {
    Literal(String),
    Expression(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    pub fn can_end_statement(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::Ident(_)
                | TokenKind::IntLit(_)
                | TokenKind::FloatLit(_)
                | TokenKind::StringLit(_)
                | TokenKind::InterpolatedString(_)
                | TokenKind::Benar
                | TokenKind::Salah
                | TokenKind::Ini
                | TokenKind::Kembali
                | TokenKind::RParen
                | TokenKind::RBrace
                | TokenKind::RBracket
                | TokenKind::Question
        )
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Struktur => write!(f, "struktur"),
            TokenKind::Sifat => write!(f, "sifat"),
            TokenKind::Terapkan => write!(f, "terapkan"),
            TokenKind::Untuk => write!(f, "untuk"),
            TokenKind::Fungsi => write!(f, "fungsi"),
            TokenKind::Jika => write!(f, "jika"),
            TokenKind::SelainItu => write!(f, "selain_itu"),
            TokenKind::Selama => write!(f, "selama"),
            TokenKind::Kembali => write!(f, "kembali"),
            TokenKind::Gagal => write!(f, "gagal"),
            TokenKind::Tangkap => write!(f, "tangkap"),
            TokenKind::JalanParalel => write!(f, "jalan_paralel"),
            TokenKind::Tunggu => write!(f, "tunggu"),
            TokenKind::Comptime => write!(f, "comptime"),
            TokenKind::Ini => write!(f, "ini"),
            TokenKind::Di => write!(f, "di"),
            TokenKind::Dalam => write!(f, "dalam"),
            TokenKind::Impor => write!(f, "impor"),
            TokenKind::Benar => write!(f, "benar"),
            TokenKind::Salah => write!(f, "salah"),
            TokenKind::IntLit(n) => write!(f, "{}", n),
            TokenKind::FloatLit(n) => write!(f, "{}", n),
            TokenKind::StringLit(s) => write!(f, "\"{}\"", s),
            TokenKind::InterpolatedString(_) => write!(f, "interpolated string"),
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Walrus => write!(f, ":="),
            TokenKind::Arrow => write!(f, "->"),
            TokenKind::Exclamation => write!(f, "!"),
            TokenKind::Question => write!(f, "?"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Comma => write!(f, ","),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::Semicolon => write!(f, ";"),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::Eq => write!(f, "="),
            TokenKind::EqEq => write!(f, "=="),
            TokenKind::NotEq => write!(f, "!="),
            TokenKind::Lt => write!(f, "<"),
            TokenKind::LtEq => write!(f, "<="),
            TokenKind::Gt => write!(f, ">"),
            TokenKind::GtEq => write!(f, ">="),
            TokenKind::AndAnd => write!(f, "&&"),
            TokenKind::OrOr => write!(f, "||"),
            TokenKind::Newline => write!(f, "<baris_baru>"),
            TokenKind::Eof => write!(f, "<akhir_berkas>"),
        }
    }
}
