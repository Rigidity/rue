use std::collections::HashSet;

use rue_ast::{AstDocument, AstNode};

use crate::{
    FormatError,
    ordering::{plan_document, plan_import_groups},
    token_stream::{TokenSpan, TokenStream},
    trivia::Trivia,
};

/// Produces a canonical, ownership-sensitive comment signature. Reorderable
/// imports and paths are keyed by their significant-token content, while the
/// role and order of each comment within that unit remain significant.
pub(crate) fn comment_signature(
    document: &AstDocument,
    stream: &TokenStream,
) -> Result<Vec<String>, FormatError> {
    let document_plan = plan_document(document, stream)?;
    let import_groups = plan_import_groups(document.syntax(), stream)?;
    let mut signature = Vec::new();
    let mut covered_gaps = HashSet::new();

    append_trivia(&mut signature, "file", "header", &document_plan.header);
    append_trivia(&mut signature, "file", "footer", &document_plan.footer);
    covered_gaps.insert(stream.tokens.len());

    let document_units: Vec<_> = document_plan
        .items
        .iter()
        .map(|item| {
            covered_gaps.insert(item.span.start().index());
            let anchor = item.identity_key.clone();
            append_trivia(&mut signature, &anchor, "leading", &item.leading);
            append_trivia(&mut signature, &anchor, "trailing", &item.trailing);
            (item.span, anchor)
        })
        .collect();

    let mut path_units = Vec::new();
    for (open, group) in &import_groups {
        let group_key = group
            .items
            .iter()
            .map(|item| item.identity_key.clone())
            .collect::<Vec<_>>()
            .join("|");
        let anchor = format!("group:{group_key}");
        append_trivia(&mut signature, &anchor, "dangling-open", &group.opening);
        append_trivia(&mut signature, &anchor, "dangling-close", &group.closing);
        covered_gaps.insert(open.index() + 1);
        let close = find_close(*open, stream)?;
        covered_gaps.insert(close.index());

        for item in &group.items {
            let anchor = format!("path:{}", item.identity_key);
            covered_gaps.insert(item.span.start().index());
            if let Some(comma) = item.comma {
                covered_gaps.insert(comma.index());
            }
            append_trivia(&mut signature, &anchor, "leading", &item.leading);
            append_trivia(&mut signature, &anchor, "before-comma", &item.before_comma);
            append_trivia(&mut signature, &anchor, "trailing", &item.trailing);
            path_units.push((item.span, anchor));
        }
    }

    for (gap_index, gap) in stream.gaps.iter().enumerate() {
        if covered_gaps.contains(&gap_index) {
            continue;
        }
        let (anchor, relative_gap) = path_units
            .iter()
            .find(|(span, _)| contains_gap(*span, gap_index))
            .or_else(|| {
                document_units
                    .iter()
                    .find(|(span, _)| contains_gap(*span, gap_index))
            })
            .map_or_else(
                || ("file".to_string(), gap_index),
                |(span, anchor)| {
                    (
                        anchor.clone(),
                        normalized_relative_gap(stream, *span, gap_index),
                    )
                },
            );
        for (order, comment) in gap.comments.iter().enumerate() {
            signature.push(format!(
                "{anchor}\0inner:{relative_gap}:{order}\0{:?}\0{}",
                comment.kind, comment.text
            ));
        }
    }

    signature.sort();
    Ok(signature)
}

fn append_trivia(signature: &mut Vec<String>, anchor: &str, role: &str, trivia: &Trivia) {
    for (order, comment) in trivia.gap.comments.iter().enumerate() {
        signature.push(format!(
            "{anchor}\0{role}:{order}\0{:?}\0{}",
            comment.kind, comment.text
        ));
    }
}

fn normalized_relative_gap(stream: &TokenStream, span: TokenSpan, gap: usize) -> usize {
    (span.start().index()..gap)
        .filter(|index| !is_optional_trailing_comma(stream, *index))
        .count()
}

fn is_optional_trailing_comma(stream: &TokenStream, index: usize) -> bool {
    stream
        .tokens
        .get(index)
        .is_some_and(|token| token.kind == rue_parser::T![,])
        && stream.tokens.get(index + 1).is_some_and(|token| {
            matches!(
                token.kind,
                rue_parser::T![')'] | rue_parser::T![']'] | rue_parser::T!['}'] | rue_parser::T![>]
            )
        })
}

fn contains_gap(span: TokenSpan, gap: usize) -> bool {
    span.start().index() < gap && gap < span.end().index()
}

fn find_close(
    open: crate::token_stream::TokenId,
    stream: &TokenStream,
) -> Result<crate::token_stream::TokenId, FormatError> {
    let mut depth = 0;
    for index in open.index()..stream.tokens.len() {
        match stream.tokens[index].kind {
            rue_parser::T!['{'] => depth += 1,
            rue_parser::T!['}'] => {
                depth -= 1;
                if depth == 0 {
                    return stream.token_id(index);
                }
            }
            _ => {}
        }
    }
    Err(FormatError::Internal(
        "import path group has no closing delimiter".to_string(),
    ))
}
