use std::fmt;

use num_derive::{FromPrimitive, ToPrimitive};

#[derive(
    Default, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, ToPrimitive, FromPrimitive,
)]
pub enum SyntaxKind {
    #[default]
    Eof,
    Unknown,
    Error,
    Whitespace,
    LineComment,
    BlockComment,

    Ident,
    Integer,
    String,

    Fn,
    If,
    Else,
    Return,
    Let,

    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,

    Plus,
    Minus,
    Star,
    Slash,

    GreaterThan,
    LessThan,
    Equals,

    Dot,
    Comma,
    Colon,
    Semicolon,

    Arrow,

    LiteralExpr,
    PrefixExpr,
    BinaryExpr,
    CallExpr,
    IfExpr,

    LetStmt,

    FnItem,
    FnParamList,
    FnParam,

    Block,
    Program,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }
}

impl fmt::Display for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => write!(f, "eof"),
            Self::Unknown => write!(f, "unknown"),
            Self::Error => write!(f, "error"),
            Self::Whitespace => write!(f, "whitespace"),
            Self::LineComment => write!(f, "line comment"),
            Self::BlockComment => write!(f, "block comment"),

            Self::Ident => write!(f, "identifier"),
            Self::Integer => write!(f, "integer"),
            Self::String => write!(f, "string"),

            Self::Fn => write!(f, "`fn`"),
            Self::If => write!(f, "`if`"),
            Self::Else => write!(f, "`else`"),
            Self::Return => write!(f, "`return`"),
            Self::Let => write!(f, "`let`"),

            Self::OpenParen => write!(f, "`(`"),
            Self::CloseParen => write!(f, "`)`"),
            Self::OpenBracket => write!(f, "`[`"),
            Self::CloseBracket => write!(f, "`]`"),
            Self::OpenBrace => write!(f, "`{{`"),
            Self::CloseBrace => write!(f, "`}}`"),

            Self::Plus => write!(f, "`+`"),
            Self::Minus => write!(f, "`-`"),
            Self::Star => write!(f, "`*`"),
            Self::Slash => write!(f, "`/`"),

            Self::GreaterThan => write!(f, "`>`"),
            Self::LessThan => write!(f, "`<`"),
            Self::Equals => write!(f, "`=`"),

            Self::Dot => write!(f, "`.`"),
            Self::Comma => write!(f, "`,`"),
            Self::Colon => write!(f, "`:`"),
            Self::Semicolon => write!(f, "`;`"),

            Self::Arrow => write!(f, "`->`"),

            Self::LiteralExpr => write!(f, "literal expression"),
            Self::PrefixExpr => write!(f, "prefix expression"),
            Self::BinaryExpr => write!(f, "binary expression"),
            Self::CallExpr => write!(f, "call expression"),
            Self::IfExpr => write!(f, "`if` expression"),

            Self::LetStmt => write!(f, "`let` statement"),

            Self::FnItem => write!(f, "`fn` item"),
            Self::FnParamList => write!(f, "parameter list"),
            Self::FnParam => write!(f, "parameter"),

            Self::Program => write!(f, "program"),
            Self::Block => write!(f, "block"),
        }
    }
}

#[macro_export]
macro_rules! T {
    [fn] => { SyntaxKind::Fn };
    [if] => { SyntaxKind::If };
    [else] => { SyntaxKind::Else };
    [return] => { SyntaxKind::Return };
    [let] => { SyntaxKind::Let };
    ['('] => { SyntaxKind::OpenParen };
    [')'] => { SyntaxKind::CloseParen };
    ['['] => { SyntaxKind::OpenBracket };
    [']'] => { SyntaxKind::CloseBracket };
    ['{'] => { SyntaxKind::OpenBrace };
    ['}'] => { SyntaxKind::CloseBrace };
    [+] => { SyntaxKind::Plus };
    [-] => { SyntaxKind::Minus };
    [*] => { SyntaxKind::Star };
    [/] => { SyntaxKind::Slash };
    [>] => { SyntaxKind::GreaterThan };
    [<] => { SyntaxKind::LessThan };
    [=] => { SyntaxKind::Equals };
    [.] => { SyntaxKind::Dot };
    [,] => { SyntaxKind::Comma };
    [:] => { SyntaxKind::Colon };
    [;] => { SyntaxKind::Semicolon };
    [->] => { SyntaxKind::Arrow };
}
