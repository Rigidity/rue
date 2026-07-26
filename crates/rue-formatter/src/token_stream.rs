use std::collections::HashMap;

use rue_parser::{SyntaxKind, SyntaxNode};

use crate::FormatError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TokenId(usize);

impl TokenId {
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    pub(crate) fn index(self) -> usize {
        self.0
    }

    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenSpan {
    pub(crate) start: TokenId,
    pub(crate) end: TokenId,
}

impl TokenSpan {
    pub(crate) fn new(start: TokenId, end: TokenId) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) kind: SyntaxKind,
    pub(crate) text: String,
    pub(crate) start: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Gap {
    pub(crate) comments: Vec<Comment>,
    pub(crate) newlines: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct Comment {
    pub(crate) text: String,
    pub(crate) kind: SyntaxKind,
    pub(crate) newlines_before: usize,
    pub(crate) placement: CommentPlacement,
    pub(crate) multiline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CommentPlacement {
    Leading,
    Trailing,
    Dangling,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenStream {
    pub(crate) tokens: Vec<Token>,
    pub(crate) gaps: Vec<Gap>,
    pub(crate) comment_count: usize,
    token_by_offset: HashMap<usize, TokenId>,
}

impl TokenStream {
    pub(crate) fn from_syntax(root: &SyntaxNode) -> Result<Self, FormatError> {
        let raw_tokens: Vec<_> = root
            .descendants_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .collect();
        let mut tokens = Vec::new();
        let mut gaps = vec![Gap::default()];
        let mut pending_newlines = 0;
        let mut line_has_significant = false;
        let mut comment_count = 0;
        let mut last_end = 0;

        for raw in raw_tokens {
            let start: usize = raw.text_range().start().into();
            let end: usize = raw.text_range().end().into();
            if start < last_end {
                return Err(FormatError::TokenOrder {
                    previous_end: last_end,
                    next_start: start,
                });
            }
            last_end = end;

            match raw.kind() {
                SyntaxKind::Whitespace => {
                    let count = newline_count(raw.text());
                    pending_newlines += count;
                    if count > 0 {
                        line_has_significant = false;
                    }
                }
                kind @ (SyntaxKind::LineComment | SyntaxKind::BlockComment) => {
                    let text = if kind == SyntaxKind::LineComment {
                        raw.text().trim_end_matches(['\r', '\n']).to_string()
                    } else {
                        raw.text().to_string()
                    };
                    let multiline =
                        kind == SyntaxKind::BlockComment && newline_count(raw.text()) > 0;
                    gaps.last_mut()
                        .expect("a leading gap always exists")
                        .comments
                        .push(Comment {
                            text,
                            kind,
                            newlines_before: pending_newlines,
                            placement: if line_has_significant && pending_newlines == 0 {
                                CommentPlacement::Trailing
                            } else {
                                CommentPlacement::Leading
                            },
                            multiline,
                        });
                    comment_count += 1;
                    pending_newlines = 0;
                    if kind == SyntaxKind::LineComment {
                        pending_newlines = newline_count(raw.text());
                        line_has_significant = false;
                    } else if multiline {
                        line_has_significant = false;
                    }
                }
                kind if kind.is_trivia() => {
                    return Err(FormatError::UnsupportedSyntax(kind));
                }
                kind => {
                    gaps.last_mut()
                        .expect("a leading gap always exists")
                        .newlines = pending_newlines;
                    pending_newlines = 0;
                    tokens.push(Token {
                        kind,
                        text: raw.text().to_string(),
                        start,
                    });
                    gaps.push(Gap::default());
                    line_has_significant = true;
                }
            }
        }

        gaps.last_mut()
            .expect("a trailing gap always exists")
            .newlines = pending_newlines;
        if gaps.len() != tokens.len() + 1 {
            return Err(FormatError::Internal(
                "token stream did not produce one more gap than tokens".to_string(),
            ));
        }

        let token_by_offset = tokens
            .iter()
            .enumerate()
            .map(|(index, token)| (token.start, TokenId::new(index)))
            .collect();

        Ok(Self {
            tokens,
            gaps,
            comment_count,
            token_by_offset,
        })
    }

    pub(crate) fn token(&self, id: TokenId) -> &Token {
        &self.tokens[id.index()]
    }

    pub(crate) fn gap_before(&self, id: TokenId) -> &Gap {
        &self.gaps[id.index()]
    }

    pub(crate) fn token_id_at_offset(&self, offset: usize) -> Result<TokenId, FormatError> {
        self.token_by_offset.get(&offset).copied().ok_or_else(|| {
            FormatError::Internal(format!(
                "syntax token at source offset {offset} has no significant token ID"
            ))
        })
    }
}

fn newline_count(text: &str) -> usize {
    text.matches('\n').count()
        + text
            .as_bytes()
            .windows(2)
            .filter(|pair| pair[0] == b'\r' && pair[1] != b'\n')
            .count()
        + usize::from(text.ends_with('\r'))
}
