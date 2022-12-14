use crate::base::Base;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenKind {
    Ident,
    String { is_terminated: bool },
    Integer { base: Base, is_empty: bool },
    DefKw,
    LetKw,
    TrueKw,
    FalseKw,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
    LessThan,
    GreaterThan,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    And,
    Or,
    Xor,
    Arrow,
    Colon,
    Semicolon,
    Comma,
    Dot,
    Exclamation,
    Equals,
    Whitespace,
    BlockComment { is_terminated: bool },
    LineComment,
    Error,
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment { .. }
        )
    }
}
