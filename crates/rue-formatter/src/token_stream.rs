use rue_parser::{SyntaxKind, SyntaxNode};

use crate::FormatError;

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
    pub(crate) trailing: bool,
    pub(crate) multiline: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TokenStream {
    pub(crate) tokens: Vec<Token>,
    pub(crate) gaps: Vec<Gap>,
    pub(crate) comment_count: usize,
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
                            trailing: line_has_significant && pending_newlines == 0,
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

        Ok(Self {
            tokens,
            gaps,
            comment_count,
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
