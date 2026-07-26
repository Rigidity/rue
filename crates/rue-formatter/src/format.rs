use std::collections::{HashMap, HashSet};

use rue_ast::{AstDocument, AstNode};
use rue_parser::{SyntaxKind, SyntaxNode, T};

use crate::{
    FormatError,
    document::Doc,
    token_stream::{Comment, Gap, TokenStream},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelimiterStyle {
    Block,
    Braced,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Separator {
    None,
    Space,
    Soft,
    Hard,
    Empty,
}

#[derive(Debug)]
struct Layout {
    pairs: HashMap<usize, usize>,
    block_openers: HashSet<usize>,
    braced_openers: HashSet<usize>,
    generic_tokens: HashSet<usize>,
    prefix_operators: HashSet<usize>,
    attached_openers: HashSet<usize>,
    item_ends: HashSet<usize>,
    groups: HashMap<usize, usize>,
    binary_operators: HashMap<usize, Vec<usize>>,
}

pub(crate) fn format_document(
    document: &AstDocument,
    stream: &TokenStream,
) -> Result<Doc, FormatError> {
    validate_node_kinds(document.syntax())?;
    let layout = Layout::new(document, stream)?;
    let mut formatter = Formatter {
        stream,
        layout,
        consumed_tokens: 0,
        consumed_comments: 0,
    };

    let mut docs = vec![formatter.gap_doc(&stream.gaps[0], Separator::None)];
    docs.push(formatter.span(0, stream.tokens.len())?);
    if !stream.tokens.is_empty() {
        docs.push(formatter.gap_doc(
            stream.gaps.last().expect("a trailing gap always exists"),
            Separator::None,
        ));
    }

    if formatter.consumed_tokens != stream.tokens.len() {
        return Err(FormatError::Internal(format!(
            "emitted {} of {} significant tokens",
            formatter.consumed_tokens,
            stream.tokens.len()
        )));
    }
    if formatter.consumed_comments != stream.comment_count {
        return Err(FormatError::Internal(format!(
            "emitted {} of {} comments",
            formatter.consumed_comments, stream.comment_count
        )));
    }

    Ok(Doc::concat(docs))
}

#[derive(Debug)]
struct Formatter<'a> {
    stream: &'a TokenStream,
    layout: Layout,
    consumed_tokens: usize,
    consumed_comments: usize,
}

impl Formatter<'_> {
    fn span(&mut self, start: usize, end: usize) -> Result<Doc, FormatError> {
        self.span_inner(start, end, true)
    }

    fn span_inner(
        &mut self,
        start: usize,
        end: usize,
        allow_outer_group: bool,
    ) -> Result<Doc, FormatError> {
        if allow_outer_group
            && self
                .layout
                .groups
                .get(&start)
                .is_some_and(|group_end| *group_end + 1 == end)
        {
            return self.grouped_span(start, end);
        }

        let mut docs = Vec::new();
        let mut index = start;

        while index < end {
            let atom_end = if let Some(&group_end) = self.layout.groups.get(&index)
                && group_end < end
                && !(index == start && group_end + 1 == end)
            {
                docs.push(self.grouped_span(index, group_end + 1)?);
                group_end
            } else if let Some(&close) = self.layout.pairs.get(&index) {
                if close >= end {
                    return Err(FormatError::Internal(
                        "delimiter pair crosses formatting span".to_string(),
                    ));
                }
                docs.push(self.delimited(index, close)?);
                close
            } else {
                docs.push(self.token(index));
                index
            };

            index = atom_end + 1;
            if index < end {
                let separator = self.separator(atom_end, index);
                docs.push(self.gap_doc(&self.stream.gaps[index], separator));
            }
        }

        Ok(Doc::concat(docs))
    }

    fn grouped_span(&mut self, start: usize, end: usize) -> Result<Doc, FormatError> {
        if self.layout.binary_operators.contains_key(&start) {
            return Ok(self.binary_span(start, end)?.group());
        }
        Ok(self.span_inner(start, end, false)?.indent().group())
    }

    fn binary_span(&mut self, start: usize, end: usize) -> Result<Doc, FormatError> {
        let operators = self
            .layout
            .binary_operators
            .get(&start)
            .cloned()
            .unwrap_or_default();

        let Some(&first_operator) = operators.first() else {
            return self.span_inner(start, end, false);
        };

        let mut docs = vec![self.span_inner(start, first_operator, false)?];
        for (position, operator) in operators.iter().enumerate() {
            let operand_start = operator + 1;
            let segment_end = operators.get(position + 1).copied().unwrap_or(end);
            let operator_doc = self.token(*operator);
            let after_operator = self.gap_doc(&self.stream.gaps[operand_start], Separator::Space);
            let operand = self.span_inner(operand_start, segment_end, false)?;
            let segment = Doc::concat([operator_doc, after_operator, operand]);
            if is_comparison_operator(self.stream.tokens[*operator].kind)
                && self.stream.gaps[*operator].comments.is_empty()
            {
                let preferred = Doc::preferred_break(segment, position == 0);
                docs.push(if position == 0 {
                    preferred
                } else {
                    preferred.indent()
                });
            } else {
                let before_operator = self.gap_doc(&self.stream.gaps[*operator], Separator::Soft);
                docs.push(Doc::concat([before_operator, segment]).indent());
            }
        }
        Ok(Doc::concat(docs))
    }

    fn delimited(&mut self, open: usize, close: usize) -> Result<Doc, FormatError> {
        let style = if self
            .layout
            .block_openers
            .contains(&self.stream.tokens[open].start)
        {
            DelimiterStyle::Block
        } else if self
            .layout
            .braced_openers
            .contains(&self.stream.tokens[open].start)
        {
            DelimiterStyle::Braced
        } else {
            DelimiterStyle::Group
        };
        let open_doc = self.token(open);
        let close_doc = self.token(close);
        let inner_start = open + 1;

        if inner_start == close {
            let gap = self.gap_doc_with_comments(
                &self.stream.gaps[inner_start],
                if self.stream.gaps[inner_start].comments.is_empty() {
                    Separator::None
                } else if style == DelimiterStyle::Block {
                    Separator::Hard
                } else {
                    Separator::Soft
                },
                true,
            );
            return Ok(Doc::concat([open_doc, gap, close_doc]).group());
        }

        let inner = self.span(inner_start, close)?;
        let leading_gap = &self.stream.gaps[inner_start];
        let trailing_gap = &self.stream.gaps[close];
        let leading_comment_starts_line = leading_gap
            .comments
            .first()
            .is_some_and(|comment| comment.newlines_before > 0 || comment.multiline);
        let trailing_comment_ends_line = trailing_gap.comments.last().is_some_and(|comment| {
            comment.kind == SyntaxKind::LineComment
                || comment.multiline
                || trailing_gap.newlines > 0
        });
        let leading = self.gap_doc_with_comments(
            leading_gap,
            match style {
                DelimiterStyle::Block => Separator::Hard,
                DelimiterStyle::Braced => Separator::Soft,
                DelimiterStyle::Group => Separator::None,
            },
            true,
        );
        let trailing = self.gap_doc(
            trailing_gap,
            match style {
                DelimiterStyle::Block => Separator::Hard,
                DelimiterStyle::Braced => Separator::Soft,
                DelimiterStyle::Group => Separator::None,
            },
        );

        Ok(match style {
            DelimiterStyle::Block => Doc::concat([
                open_doc,
                Doc::concat([leading, inner]).indent(),
                trailing,
                close_doc,
            ]),
            DelimiterStyle::Braced => Doc::concat([
                open_doc,
                Doc::concat([leading, inner]).indent(),
                trailing,
                close_doc,
            ])
            .group(),
            DelimiterStyle::Group => Doc::concat([
                open_doc,
                Doc::concat([
                    if leading_comment_starts_line {
                        Doc::Nil
                    } else {
                        Doc::if_break(Doc::hard_line(), Doc::Nil)
                    },
                    leading,
                    inner,
                ])
                .indent(),
                trailing,
                if trailing_comment_ends_line {
                    Doc::Nil
                } else {
                    Doc::if_break(Doc::hard_line(), Doc::Nil)
                },
                close_doc,
            ])
            .group(),
        })
    }

    fn token(&mut self, index: usize) -> Doc {
        self.consumed_tokens += 1;
        Doc::text(self.stream.tokens[index].text.clone())
    }

    fn separator(&self, previous: usize, next: usize) -> Separator {
        let left = &self.stream.tokens[previous];
        let right = &self.stream.tokens[next];

        if self.layout.item_ends.contains(&left.start) {
            return Separator::Empty;
        }

        if left.kind == T![;] {
            return Separator::Hard;
        }
        if left.kind == T![,] {
            return Separator::Soft;
        }
        if no_space_after(left.kind) || no_space_before(right.kind) {
            return Separator::None;
        }
        if SyntaxKind::BINARY_OPS.contains(&left.kind)
            && !self.layout.generic_tokens.contains(&left.start)
            && !self.layout.prefix_operators.contains(&left.start)
        {
            return Separator::Soft;
        }
        if right.kind == T!['('] && self.layout.attached_openers.contains(&right.start) {
            return Separator::None;
        }
        if left.kind == T!['}'] && right.kind == T![else] {
            return Separator::Space;
        }

        if ((left.kind == T![<]
            || right.kind == T![<]
            || right.kind == T![>]
            || (left.kind == T![>] && right.kind == T!['(']))
            && (self.layout.generic_tokens.contains(&left.start)
                || self.layout.generic_tokens.contains(&right.start)))
            || (SyntaxKind::PREFIX_OPS.contains(&left.kind)
                && self.layout.prefix_operators.contains(&left.start))
        {
            Separator::None
        } else {
            Separator::Space
        }
    }

    fn gap_doc(&mut self, gap: &Gap, requested: Separator) -> Doc {
        self.gap_doc_with_comments(gap, requested, false)
    }

    fn gap_doc_with_comments(
        &mut self,
        gap: &Gap,
        requested: Separator,
        ignore_trailing: bool,
    ) -> Doc {
        if gap.comments.is_empty() {
            if gap.newlines > 1 && requested != Separator::None {
                return Doc::empty_line();
            }
            return separator_doc(requested);
        }

        let mut docs = Vec::new();
        for (index, comment) in gap.comments.iter().enumerate() {
            self.consumed_comments += 1;
            let before = if index == 0 {
                comment_separator(comment, requested, ignore_trailing)
            } else {
                comment_separator(comment, Separator::Hard, false)
            };
            docs.push(before);
            docs.push(Doc::text(comment.text.clone()));

            let followed_inline = match gap.comments.get(index + 1) {
                Some(next) => next.newlines_before == 0,
                None => gap.newlines == 0,
            };
            if comment.kind == SyntaxKind::BlockComment && !comment.multiline && followed_inline {
                docs.push(Doc::space());
            }
        }

        let last = gap.comments.last().expect("comments are not empty");
        if gap.newlines > 1 {
            docs.push(Doc::empty_line());
        } else if last.kind == SyntaxKind::LineComment || last.multiline || gap.newlines > 0 {
            docs.push(Doc::hard_line());
        }
        Doc::concat(docs)
    }
}

impl Layout {
    fn new(document: &AstDocument, stream: &TokenStream) -> Result<Self, FormatError> {
        let mut pairs = HashMap::new();
        let mut stack: Vec<(SyntaxKind, usize)> = Vec::new();
        for (index, token) in stream.tokens.iter().enumerate() {
            match token.kind {
                T!['('] | T!['['] | T!['{'] => stack.push((token.kind, index)),
                T![')'] | T![']'] | T!['}'] => {
                    let Some((open, open_index)) = stack.pop() else {
                        return Err(FormatError::Internal(
                            "closing delimiter has no opener".to_string(),
                        ));
                    };
                    if !delimiters_match(open, token.kind) {
                        return Err(FormatError::Internal(
                            "mismatched delimiters in valid syntax".to_string(),
                        ));
                    }
                    pairs.insert(open_index, index);
                }
                _ => {}
            }
        }
        if !stack.is_empty() {
            return Err(FormatError::Internal(
                "opening delimiter has no closer".to_string(),
            ));
        }

        let root = document.syntax();
        let mut block_openers = HashSet::new();
        let mut braced_openers = HashSet::new();
        let mut generic_tokens = HashSet::new();
        let mut prefix_operators = HashSet::new();
        let mut attached_openers = HashSet::new();

        for node in root.descendants() {
            match node.kind() {
                SyntaxKind::Block | SyntaxKind::ModuleItem | SyntaxKind::StructItem => {
                    if let Some(token) =
                        significant_tokens(&node).find(|token| token.kind() == T!['{'])
                    {
                        block_openers.insert(usize::from(token.text_range().start()));
                    }
                }
                SyntaxKind::StructInitializerExpr | SyntaxKind::StructBinding => {
                    if let Some(token) =
                        significant_tokens(&node).find(|token| token.kind() == T!['{'])
                    {
                        braced_openers.insert(usize::from(token.text_range().start()));
                    }
                }
                SyntaxKind::GenericParameters | SyntaxKind::GenericArguments => {
                    generic_tokens.extend(
                        significant_tokens(&node)
                            .filter(|token| is_generic_punctuation(token.kind()))
                            .map(|token| usize::from(token.text_range().start())),
                    );
                }
                SyntaxKind::PrefixExpr => {
                    if let Some(token) = significant_tokens(&node)
                        .find(|token| SyntaxKind::PREFIX_OPS.contains(&token.kind()))
                    {
                        prefix_operators.insert(usize::from(token.text_range().start()));
                    }
                }
                SyntaxKind::FunctionItem
                | SyntaxKind::FunctionCallExpr
                | SyntaxKind::LambdaExpr
                | SyntaxKind::LambdaType => {
                    if let Some(token) = node
                        .children_with_tokens()
                        .filter_map(rowan::NodeOrToken::into_token)
                        .find(|token| token.kind() == T!['('])
                    {
                        attached_openers.insert(usize::from(token.text_range().start()));
                    }
                }
                _ => {}
            }
        }

        let mut item_ends = HashSet::new();
        for item in document.items() {
            if let Some(token) = significant_tokens(item.syntax()).last() {
                let token_start = usize::from(token.text_range().start());
                let item_end = stream
                    .tokens
                    .iter()
                    .position(|candidate| candidate.start == token_start)
                    .and_then(|index| stream.tokens.get(index + 1))
                    .filter(|next| next.kind == T![;])
                    .map_or(token_start, |semicolon| semicolon.start);
                item_ends.insert(item_end);
            }
        }

        let token_indices: HashMap<usize, usize> = stream
            .tokens
            .iter()
            .enumerate()
            .map(|(index, token)| (token.start, index))
            .collect();
        let mut groups = HashMap::new();
        let mut binary_operators = HashMap::new();
        for node in root
            .descendants()
            .filter(|node| matches!(node.kind(), SyntaxKind::BinaryExpr | SyntaxKind::UnionType))
            .filter(|node| {
                node.parent()
                    .is_none_or(|parent| parent.kind() != node.kind())
            })
        {
            let mut tokens = significant_tokens(&node);
            let Some(first) = tokens.next() else {
                continue;
            };
            let last = tokens.last().unwrap_or_else(|| first.clone());
            let Some(&start) = token_indices.get(&usize::from(first.text_range().start())) else {
                continue;
            };
            let Some(&end) = token_indices.get(&usize::from(last.text_range().start())) else {
                continue;
            };
            groups
                .entry(start)
                .and_modify(|current: &mut usize| *current = (*current).max(end))
                .or_insert(end);
            if node.kind() == SyntaxKind::BinaryExpr {
                let mut operators = Vec::new();
                collect_binary_operator_starts(&node, &mut operators);
                binary_operators.insert(
                    start,
                    operators
                        .into_iter()
                        .filter_map(|operator| token_indices.get(&operator).copied())
                        .collect(),
                );
            }
        }

        Ok(Self {
            pairs,
            block_openers,
            braced_openers,
            generic_tokens,
            prefix_operators,
            attached_openers,
            item_ends,
            groups,
            binary_operators,
        })
    }
}

fn collect_binary_operator_starts(node: &SyntaxNode, operators: &mut Vec<usize>) {
    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(child) if child.kind() == SyntaxKind::BinaryExpr => {
                collect_binary_operator_starts(&child, operators);
            }
            rowan::NodeOrToken::Token(token) if SyntaxKind::BINARY_OPS.contains(&token.kind()) => {
                operators.push(usize::from(token.text_range().start()));
            }
            _ => {}
        }
    }
}

fn significant_tokens(node: &SyntaxNode) -> impl Iterator<Item = rue_parser::SyntaxToken> + '_ {
    node.descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
}

fn separator_doc(separator: Separator) -> Doc {
    match separator {
        Separator::None => Doc::Nil,
        Separator::Space => Doc::space(),
        Separator::Soft => Doc::soft_line(),
        Separator::Hard => Doc::hard_line(),
        Separator::Empty => Doc::empty_line(),
    }
}

fn comment_separator(comment: &Comment, requested: Separator, ignore_trailing: bool) -> Doc {
    if comment.trailing && !ignore_trailing {
        return Doc::space();
    }
    if comment.newlines_before > 1 {
        return Doc::empty_line();
    }
    if comment.newlines_before > 0 || comment.multiline {
        return Doc::hard_line();
    }
    separator_doc(match requested {
        Separator::None => Separator::None,
        requested => requested,
    })
}

fn no_space_after(kind: SyntaxKind) -> bool {
    matches!(kind, T!['('] | T!['['] | T![::] | T![.] | T![...] | T![,])
}

fn no_space_before(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        T![')'] | T![']'] | T![,] | T![;] | T![::] | T![.] | T![:]
    )
}

fn is_generic_punctuation(kind: SyntaxKind) -> bool {
    matches!(kind, T![<] | T![>] | T![,])
}

fn is_comparison_operator(kind: SyntaxKind) -> bool {
    matches!(kind, T![==] | T![!=] | T![<] | T![>] | T![<=] | T![>=])
}

fn delimiters_match(open: SyntaxKind, close: SyntaxKind) -> bool {
    matches!(
        (open, close),
        (T!['('], T![')']) | (T!['['], T![']']) | (T!['{'], T!['}'])
    )
}

fn validate_node_kinds(root: &SyntaxNode) -> Result<(), FormatError> {
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::Document
            | SyntaxKind::ModuleItem
            | SyntaxKind::FunctionItem
            | SyntaxKind::FunctionParameter
            | SyntaxKind::ConstantItem
            | SyntaxKind::TypeAliasItem
            | SyntaxKind::StructItem
            | SyntaxKind::StructField
            | SyntaxKind::ImportItem
            | SyntaxKind::ImportPath
            | SyntaxKind::ImportPathSegment
            | SyntaxKind::GenericParameters
            | SyntaxKind::GenericArguments
            | SyntaxKind::LiteralType
            | SyntaxKind::PathType
            | SyntaxKind::UnionType
            | SyntaxKind::GroupType
            | SyntaxKind::PairType
            | SyntaxKind::ListType
            | SyntaxKind::ListTypeItem
            | SyntaxKind::LambdaType
            | SyntaxKind::LambdaParameter
            | SyntaxKind::Block
            | SyntaxKind::LetStmt
            | SyntaxKind::ExprStmt
            | SyntaxKind::IfStmt
            | SyntaxKind::ReturnStmt
            | SyntaxKind::AssertStmt
            | SyntaxKind::RaiseStmt
            | SyntaxKind::DebugStmt
            | SyntaxKind::PathExpr
            | SyntaxKind::PathSegment
            | SyntaxKind::StructInitializerExpr
            | SyntaxKind::StructInitializerField
            | SyntaxKind::LiteralExpr
            | SyntaxKind::ConstExpr
            | SyntaxKind::GroupExpr
            | SyntaxKind::PairExpr
            | SyntaxKind::ListExpr
            | SyntaxKind::ListItem
            | SyntaxKind::PrefixExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::FunctionCallExpr
            | SyntaxKind::IfExpr
            | SyntaxKind::GuardExpr
            | SyntaxKind::CastExpr
            | SyntaxKind::FieldAccessExpr
            | SyntaxKind::LambdaExpr
            | SyntaxKind::NamedBinding
            | SyntaxKind::PairBinding
            | SyntaxKind::ListBinding
            | SyntaxKind::ListBindingItem
            | SyntaxKind::StructBinding
            | SyntaxKind::StructFieldBinding => {}
            kind => return Err(FormatError::UnsupportedSyntax(kind)),
        }
    }
    Ok(())
}
