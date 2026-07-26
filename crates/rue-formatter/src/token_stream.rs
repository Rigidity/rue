use std::collections::HashMap;

use rue_parser::{SyntaxKind, SyntaxNode};

use crate::FormatError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TokenId(usize);

impl TokenId {
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct TokenBoundary(usize);

impl TokenBoundary {
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenSpan {
    start: TokenId,
    end: TokenBoundary,
}

impl TokenSpan {
    pub(crate) fn start(self) -> TokenId {
        self.start
    }

    pub(crate) fn end(self) -> TokenBoundary {
        self.end
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
    tokens: Vec<Token>,
    gaps: Vec<Gap>,
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
                    return Err(FormatError::UnsupportedTrivia(kind));
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
            .map(|(index, token)| (token.start, TokenId(index)))
            .collect();

        Ok(Self {
            tokens,
            gaps,
            comment_count,
            token_by_offset,
        })
    }

    pub(crate) fn token(&self, id: TokenId) -> &Token {
        self.tokens
            .get(id.index())
            .expect("TokenId was validated by this token stream")
    }

    pub(crate) fn len(&self) -> usize {
        self.tokens.len()
    }

    pub(crate) fn token_ids(&self) -> impl Iterator<Item = (TokenId, &Token)> {
        self.tokens
            .iter()
            .enumerate()
            .map(|(index, token)| (TokenId(index), token))
    }

    pub(crate) fn tokens_in(&self, span: TokenSpan) -> &[Token] {
        self.tokens
            .get(span.start().index()..span.end().index())
            .expect("TokenSpan was validated by this token stream")
    }

    pub(crate) fn gap_before(&self, id: TokenId) -> &Gap {
        self.gap(TokenBoundary(id.index()))
    }

    pub(crate) fn gap(&self, boundary: TokenBoundary) -> &Gap {
        self.gaps
            .get(boundary.index())
            .expect("TokenBoundary was validated by this token stream")
    }

    pub(crate) fn first_gap(&self) -> &Gap {
        self.gap(TokenBoundary(0))
    }

    pub(crate) fn final_gap(&self) -> &Gap {
        self.gaps
            .last()
            .expect("a token stream always has a final gap")
    }

    pub(crate) fn gaps(&self) -> impl Iterator<Item = (TokenBoundary, &Gap)> {
        self.gaps
            .iter()
            .enumerate()
            .map(|(index, gap)| (TokenBoundary(index), gap))
    }

    pub(crate) fn end_boundary(&self) -> TokenBoundary {
        TokenBoundary(self.tokens.len())
    }

    pub(crate) fn token_id(&self, index: usize) -> Result<TokenId, FormatError> {
        (index < self.tokens.len())
            .then_some(TokenId(index))
            .ok_or_else(|| {
                FormatError::Internal(format!(
                    "token index {index} is out of range for {} tokens",
                    self.tokens.len()
                ))
            })
    }

    pub(crate) fn boundary(&self, index: usize) -> Result<TokenBoundary, FormatError> {
        (index <= self.tokens.len())
            .then_some(TokenBoundary(index))
            .ok_or_else(|| {
                FormatError::Internal(format!(
                    "token boundary {index} is out of range for {} tokens",
                    self.tokens.len()
                ))
            })
    }

    pub(crate) fn boundary_before(&self, id: TokenId) -> TokenBoundary {
        let _ = self.token(id);
        TokenBoundary(id.index())
    }

    pub(crate) fn boundary_after(&self, id: TokenId) -> TokenBoundary {
        let _ = self.token(id);
        TokenBoundary(id.index() + 1)
    }

    pub(crate) fn token_at(&self, boundary: TokenBoundary) -> Option<TokenId> {
        (boundary.index() < self.tokens.len()).then_some(TokenId(boundary.index()))
    }

    pub(crate) fn next_token(&self, id: TokenId) -> Option<TokenId> {
        self.token_at(self.boundary_after(id))
    }

    pub(crate) fn previous_token(&self, id: TokenId) -> Option<TokenId> {
        let _ = self.token(id);
        id.index().checked_sub(1).map(TokenId)
    }

    pub(crate) fn span(
        &self,
        start: TokenId,
        end: TokenBoundary,
    ) -> Result<TokenSpan, FormatError> {
        if end.index() > self.tokens.len() {
            return Err(FormatError::Internal(format!(
                "span end {} is out of range for {} tokens",
                end.index(),
                self.tokens.len()
            )));
        }
        if start.index() >= end.index() {
            return Err(FormatError::Internal(format!(
                "token span start {} must precede end {}",
                start.index(),
                end.index()
            )));
        }
        Ok(TokenSpan { start, end })
    }

    pub(crate) fn span_through(
        &self,
        start: TokenId,
        last: TokenId,
    ) -> Result<TokenSpan, FormatError> {
        self.span(start, self.boundary(last.index() + 1)?)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_stream() -> TokenStream {
        let tokens = ["a", "b", "c"]
            .into_iter()
            .enumerate()
            .map(|(start, text)| Token {
                kind: SyntaxKind::Ident,
                text: text.to_string(),
                start,
            })
            .collect::<Vec<_>>();
        TokenStream {
            token_by_offset: tokens
                .iter()
                .enumerate()
                .map(|(index, token)| (token.start, TokenId(index)))
                .collect(),
            gaps: vec![Gap::default(); tokens.len() + 1],
            tokens,
            comment_count: 0,
        }
    }

    #[test]
    fn constructs_valid_bounded_spans() {
        let stream = test_stream();
        let first = stream.token_id(0).unwrap();
        let second = stream.token_id(1).unwrap();
        let span = stream.span_through(first, second).unwrap();

        assert_eq!(span.start(), first);
        assert_eq!(span.end(), stream.boundary(2).unwrap());
        assert_eq!(stream.token(span.start()).text, "a");
        assert_eq!(
            stream.token_at(span.end()),
            Some(stream.token_id(2).unwrap())
        );
    }

    #[test]
    fn end_boundary_is_not_a_token_id() {
        let stream = test_stream();
        let end = stream.boundary(stream.tokens.len()).unwrap();
        let span = stream
            .span(stream.token_id(0).unwrap(), end)
            .expect("end-of-stream is a valid exclusive span boundary");

        assert_eq!(span.end(), end);
        assert_eq!(stream.token_at(end), None);
        assert_eq!(stream.gap(end).comments.len(), 0);
        assert!(stream.token_id(stream.tokens.len()).is_err());
    }

    #[test]
    fn rejects_reversed_and_empty_spans() {
        let stream = test_stream();
        let first = stream.token_id(0).unwrap();
        let last = stream.token_id(2).unwrap();

        assert!(stream.span(last, stream.boundary_before(first)).is_err());
        assert!(stream.span(first, stream.boundary_before(first)).is_err());
    }

    #[test]
    fn rejects_out_of_range_ids_and_boundaries() {
        let stream = test_stream();
        assert!(stream.token_id(3).is_err());
        assert!(stream.boundary(4).is_err());
    }
}
