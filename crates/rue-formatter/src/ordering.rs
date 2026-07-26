use std::{collections::HashMap, fmt::Write};

use rue_ast::{AstDocument, AstItem, AstNode};
use rue_parser::{SyntaxKind, SyntaxNode, T};

use crate::{
    FormatError,
    token_stream::{TokenId, TokenSpan, TokenStream},
    trivia::{Trivia, split_between, split_file_header, split_group_opening},
};

#[derive(Debug, Clone)]
pub(crate) struct DocumentItem {
    pub(crate) span: TokenSpan,
    pub(crate) import_group: Option<usize>,
    pub(crate) compact_group: Option<SyntaxKind>,
    pub(crate) leading: Trivia,
    pub(crate) trailing: Trivia,
    pub(crate) identity_key: String,
    sort_key: String,
    original_index: usize,
}

#[derive(Debug)]
pub(crate) struct DocumentPlan {
    pub(crate) header: Trivia,
    pub(crate) items: Vec<DocumentItem>,
    pub(crate) footer: Trivia,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportPathItem {
    pub(crate) span: TokenSpan,
    pub(crate) comma: Option<TokenId>,
    pub(crate) leading: Trivia,
    pub(crate) before_comma: Trivia,
    pub(crate) trailing: Trivia,
    pub(crate) identity_key: String,
    sort_key: String,
    original_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportGroupPlan {
    pub(crate) opening: Trivia,
    pub(crate) items: Vec<ImportPathItem>,
    pub(crate) closing: Trivia,
}

pub(crate) fn plan_document(
    document: &AstDocument,
    stream: &TokenStream,
) -> Result<DocumentPlan, FormatError> {
    let mut items = Vec::new();
    let mut next_import_group = 0;
    let mut previous_was_import = false;

    for (original_index, item) in document.items().enumerate() {
        let span = item_span(item.syntax(), stream)?;
        let is_import = matches!(&item, AstItem::ImportItem(_));
        let import_group = is_import.then(|| {
            if !previous_was_import
                || (span.start().index() > 0 && gap_has_blank_line(stream.gap_before(span.start())))
            {
                next_import_group += 1;
            }
            next_import_group
        });
        previous_was_import = is_import;

        let compact_group = matches!(
            item.syntax().kind(),
            SyntaxKind::ConstantItem | SyntaxKind::TypeAliasItem
        )
        .then_some(item.syntax().kind())
        .filter(|_| !item.syntax().text().to_string().contains('\n'));
        let path_key = stream
            .tokens_in(span)
            .iter()
            .filter(|token| !matches!(token.kind, T![import] | T![export] | T![;]))
            .map(|token| token.text.as_str())
            .collect::<String>();
        let keyword_key = stream.token(span.start()).text.as_str();

        items.push(DocumentItem {
            span,
            import_group,
            compact_group,
            leading: Trivia::default(),
            trailing: Trivia::default(),
            identity_key: canonical_node_key(item.syntax()),
            sort_key: format!("{path_key}\0{keyword_key}"),
            original_index,
        });
    }

    if items.is_empty() {
        return Ok(DocumentPlan {
            header: Trivia::new(stream.first_gap().clone()),
            items,
            footer: Trivia::default(),
        });
    }

    let (header, first_leading) = split_file_header(stream.gap_before(items[0].span.start()));
    items[0].leading = first_leading;
    for index in 1..items.len() {
        let (trailing, leading) = split_between(stream.gap_before(items[index].span.start()));
        items[index - 1].trailing = trailing;
        items[index].leading = leading;
    }
    let last = items
        .last_mut()
        .expect("non-empty document plan has a final item");
    let (trailing, footer) = split_between(stream.final_gap());
    last.trailing = trailing;

    items.sort_by(
        |left, right| match (left.import_group, right.import_group) {
            (Some(left_group), Some(right_group)) => left_group
                .cmp(&right_group)
                .then_with(|| left.sort_key.cmp(&right.sort_key))
                .then_with(|| left.original_index.cmp(&right.original_index)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.original_index.cmp(&right.original_index),
        },
    );

    Ok(DocumentPlan {
        header,
        items,
        footer,
    })
}

pub(crate) fn plan_import_groups(
    root: &SyntaxNode,
    stream: &TokenStream,
) -> Result<HashMap<TokenId, ImportGroupPlan>, FormatError> {
    let mut groups = HashMap::new();
    for node in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::ImportPathSegment)
    {
        let Some(open_token) = node
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
            .find(|token| token.kind() == T!['{'])
        else {
            continue;
        };
        let open = stream.token_id_at_offset(usize::from(open_token.text_range().start()))?;
        let close = find_matching_close(open, stream)?;
        let paths: Vec<_> = node
            .children()
            .filter(|child| child.kind() == SyntaxKind::ImportPath)
            .collect();
        if paths.is_empty() {
            continue;
        }

        let mut items = Vec::new();
        for (original_index, path) in paths.iter().enumerate() {
            let span = node_span(path, stream)?;
            let comma = stream
                .token_at(span.end())
                .filter(|id| stream.token(*id).kind == T![,]);
            let sort_key = stream
                .tokens_in(span)
                .iter()
                .map(|token| token.text.as_str())
                .collect();
            let before_comma = comma
                .map(|comma| Trivia::new(stream.gap_before(comma).clone()))
                .unwrap_or_default();
            items.push(ImportPathItem {
                span,
                comma,
                leading: Trivia::default(),
                before_comma,
                trailing: Trivia::default(),
                identity_key: canonical_node_key(path),
                sort_key,
                original_index,
            });
        }

        let (opening, first_leading) =
            split_group_opening(stream.gap_before(items[0].span.start()));
        items[0].leading = first_leading;
        for index in 1..items.len() {
            let boundary = stream.gap_before(items[index].span.start());
            let (trailing, leading) = split_between(boundary);
            items[index - 1].trailing = trailing;
            items[index].leading = leading;
        }
        let final_boundary = stream.gap_before(close);
        let (trailing, closing) = split_between(final_boundary);
        items
            .last_mut()
            .expect("non-empty import group has a final path")
            .trailing = trailing;

        items.sort_by(|left, right| {
            left.sort_key
                .cmp(&right.sort_key)
                .then_with(|| left.original_index.cmp(&right.original_index))
        });
        groups.insert(
            open,
            ImportGroupPlan {
                opening,
                items,
                closing: Trivia::dangling(closing.gap),
            },
        );
    }
    Ok(groups)
}

fn item_span(node: &SyntaxNode, stream: &TokenStream) -> Result<TokenSpan, FormatError> {
    let span = node_span(node, stream)?;
    let Some(semicolon) = stream
        .token_at(span.end())
        .filter(|id| stream.token(*id).kind == T![;])
    else {
        return Ok(span);
    };
    stream.span_through(span.start(), semicolon)
}

fn node_span(node: &SyntaxNode, stream: &TokenStream) -> Result<TokenSpan, FormatError> {
    let mut tokens = significant_tokens(node);
    let first = tokens.next().ok_or_else(|| {
        FormatError::Internal(format!("{:?} node has no significant tokens", node.kind()))
    })?;
    let last = tokens.last().unwrap_or_else(|| first.clone());
    let start = stream.token_id_at_offset(usize::from(first.text_range().start()))?;
    let last = stream.token_id_at_offset(usize::from(last.text_range().start()))?;
    stream.span_through(start, last)
}

fn find_matching_close(open: TokenId, stream: &TokenStream) -> Result<TokenId, FormatError> {
    let mut depth = 0;
    for (id, token) in stream.token_ids().skip(open.index()) {
        match token.kind {
            T!['{'] => depth += 1,
            T!['}'] => {
                depth -= 1;
                if depth == 0 {
                    return Ok(id);
                }
            }
            _ => {}
        }
    }
    Err(FormatError::Internal(
        "import path group has no closing delimiter".to_string(),
    ))
}

fn gap_has_blank_line(gap: &crate::token_stream::Gap) -> bool {
    gap.newlines > 1
        || gap
            .comments
            .iter()
            .any(|comment| comment.newlines_before > 1)
}

fn significant_tokens(node: &SyntaxNode) -> impl Iterator<Item = rue_parser::SyntaxToken> + '_ {
    node.descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
        .filter(|token| !token.kind().is_trivia())
}

fn canonical_node_key(node: &SyntaxNode) -> String {
    let mut key = format!("n:{:?}[", node.kind());
    let mut sorted_paths = if node.kind() == SyntaxKind::ImportPathSegment {
        let mut paths: Vec<_> = node
            .children()
            .filter(|child| child.kind() == SyntaxKind::ImportPath)
            .map(|child| canonical_node_key(&child))
            .collect();
        paths.sort();
        paths.into_iter()
    } else {
        Vec::new().into_iter()
    };

    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(child)
                if node.kind() == SyntaxKind::ImportPathSegment
                    && child.kind() == SyntaxKind::ImportPath =>
            {
                key.push_str(
                    &sorted_paths
                        .next()
                        .expect("each import path has a canonical identity"),
                );
            }
            rowan::NodeOrToken::Node(child) => key.push_str(&canonical_node_key(&child)),
            rowan::NodeOrToken::Token(token)
                if !token.kind().is_trivia() && token.kind() != T![,] =>
            {
                write!(&mut key, "t:{:?}:{};", token.kind(), token.text())
                    .expect("writing to a String cannot fail");
            }
            rowan::NodeOrToken::Token(_) => {}
        }
    }
    key.push(']');
    key
}
