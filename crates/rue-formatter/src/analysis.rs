use rue_ast::{AstDocument, AstNode};
use rue_parser::{SyntaxKind, SyntaxNode, T};

use crate::{
    FormatError,
    ordering::{DocumentPlan, ImportGroupPlan, plan_document, plan_import_groups},
    token_stream::{TokenId, TokenStream},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DelimiterStyle {
    Block,
    Braced,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemBoundary {
    Document,
    Module,
    Block,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TokenFacts {
    pub(crate) pair: Option<TokenId>,
    pub(crate) delimiter_style: Option<DelimiterStyle>,
    pub(crate) item_boundary: Option<ItemBoundary>,
    pub(crate) group_end: Option<TokenId>,
    pub(crate) binary_operators: Vec<TokenId>,
    flags: TokenFlags,
}

#[derive(Debug, Clone, Copy, Default)]
struct TokenFlags(u8);

impl TokenFlags {
    const TRAILING_COMMA: u8 = 1 << 0;
    const GENERIC: u8 = 1 << 1;
    const PREFIX_OPERATOR: u8 = 1 << 2;
    const ATTACHED_OPENER: u8 = 1 << 3;
    const ABSOLUTE_PATH_START: u8 = 1 << 4;

    fn insert(&mut self, flag: u8) {
        self.0 |= flag;
    }

    fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
}

impl TokenFacts {
    pub(crate) fn supports_trailing_comma(&self) -> bool {
        self.flags.contains(TokenFlags::TRAILING_COMMA)
    }

    pub(crate) fn is_generic(&self) -> bool {
        self.flags.contains(TokenFlags::GENERIC)
    }

    pub(crate) fn is_prefix_operator(&self) -> bool {
        self.flags.contains(TokenFlags::PREFIX_OPERATOR)
    }

    pub(crate) fn is_attached_opener(&self) -> bool {
        self.flags.contains(TokenFlags::ATTACHED_OPENER)
    }

    pub(crate) fn is_absolute_path_start(&self) -> bool {
        self.flags.contains(TokenFlags::ABSOLUTE_PATH_START)
    }
}

#[derive(Debug)]
pub(crate) struct Layout {
    facts: Vec<TokenFacts>,
    pub(crate) document: DocumentPlan,
    pub(crate) import_groups: std::collections::HashMap<TokenId, ImportGroupPlan>,
}

impl Layout {
    pub(crate) fn new(document: &AstDocument, stream: &TokenStream) -> Result<Self, FormatError> {
        let mut facts = vec![TokenFacts::default(); stream.tokens.len()];
        pair_delimiters(stream, &mut facts)?;
        analyze_nodes(document.syntax(), stream, &mut facts)?;
        analyze_boundaries(document, stream, &mut facts)?;
        analyze_expression_groups(document.syntax(), stream, &mut facts)?;

        Ok(Self {
            facts,
            document: plan_document(document, stream)?,
            import_groups: plan_import_groups(document.syntax(), stream)?,
        })
    }

    pub(crate) fn facts(&self, token: TokenId) -> &TokenFacts {
        &self.facts[token.index()]
    }
}

fn pair_delimiters(stream: &TokenStream, facts: &mut [TokenFacts]) -> Result<(), FormatError> {
    let mut stack: Vec<(SyntaxKind, TokenId)> = Vec::new();
    for (index, token) in stream.tokens.iter().enumerate() {
        let id = TokenId::new(index);
        match token.kind {
            T!['('] | T!['['] | T!['{'] => stack.push((token.kind, id)),
            T![')'] | T![']'] | T!['}'] => {
                let Some((open, open_id)) = stack.pop() else {
                    return Err(FormatError::Internal(
                        "closing delimiter has no opener".to_string(),
                    ));
                };
                if !delimiters_match(open, token.kind) {
                    return Err(FormatError::Internal(
                        "mismatched delimiters in valid syntax".to_string(),
                    ));
                }
                facts[open_id.index()].pair = Some(id);
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(FormatError::Internal(
            "opening delimiter has no closer".to_string(),
        ));
    }
    Ok(())
}

fn analyze_nodes(
    root: &SyntaxNode,
    stream: &TokenStream,
    facts: &mut [TokenFacts],
) -> Result<(), FormatError> {
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::Block | SyntaxKind::ModuleItem | SyntaxKind::StructItem => {
                if let Some(id) = direct_or_descendant_token(&node, T!['{'], stream)? {
                    facts[id.index()].delimiter_style = Some(DelimiterStyle::Block);
                    if node.kind() == SyntaxKind::StructItem {
                        facts[id.index()].flags.insert(TokenFlags::TRAILING_COMMA);
                    }
                }
            }
            SyntaxKind::StructInitializerExpr | SyntaxKind::StructBinding => {
                if let Some(id) = direct_or_descendant_token(&node, T!['{'], stream)? {
                    facts[id.index()].delimiter_style = Some(DelimiterStyle::Braced);
                    facts[id.index()].flags.insert(TokenFlags::TRAILING_COMMA);
                }
            }
            SyntaxKind::PairExpr
            | SyntaxKind::PairType
            | SyntaxKind::PairBinding
            | SyntaxKind::ListExpr
            | SyntaxKind::ListType
            | SyntaxKind::ListBinding
            | SyntaxKind::ImportPathSegment => {
                if let Some(token) = node
                    .children_with_tokens()
                    .filter_map(rowan::NodeOrToken::into_token)
                    .find(|token| matches!(token.kind(), T!['('] | T!['['] | T!['{']))
                {
                    let id = token_id(&token, stream)?;
                    facts[id.index()].flags.insert(TokenFlags::TRAILING_COMMA);
                }
            }
            SyntaxKind::GenericParameters | SyntaxKind::GenericArguments => {
                let direct_tokens: Vec<_> = node
                    .children_with_tokens()
                    .filter_map(rowan::NodeOrToken::into_token)
                    .filter(|token| !token.kind().is_trivia())
                    .collect();
                for token in
                    significant_tokens(&node).filter(|token| is_generic_punctuation(token.kind()))
                {
                    let id = token_id(&token, stream)?;
                    facts[id.index()].flags.insert(TokenFlags::GENERIC);
                }
                if let (Some(open), Some(close)) = (direct_tokens.first(), direct_tokens.last())
                    && open.kind() == T![<]
                    && close.kind() == T![>]
                {
                    let open = token_id(open, stream)?;
                    let close = token_id(close, stream)?;
                    facts[open.index()].flags.insert(TokenFlags::TRAILING_COMMA);
                    facts[open.index()].pair = Some(close);
                }
            }
            SyntaxKind::PrefixExpr => {
                if let Some(token) = significant_tokens(&node)
                    .find(|token| SyntaxKind::PREFIX_OPS.contains(&token.kind()))
                {
                    let id = token_id(&token, stream)?;
                    facts[id.index()].flags.insert(TokenFlags::PREFIX_OPERATOR);
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
                    let id = token_id(&token, stream)?;
                    facts[id.index()].flags.insert(TokenFlags::ATTACHED_OPENER);
                    facts[id.index()].flags.insert(TokenFlags::TRAILING_COMMA);
                }
            }
            SyntaxKind::PathExpr | SyntaxKind::PathType | SyntaxKind::ImportPath => {
                if let Some(token) = significant_tokens(&node).next()
                    && token.kind() == T![::]
                {
                    let id = token_id(&token, stream)?;
                    facts[id.index()]
                        .flags
                        .insert(TokenFlags::ABSOLUTE_PATH_START);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn analyze_boundaries(
    document: &AstDocument,
    stream: &TokenStream,
    facts: &mut [TokenFacts],
) -> Result<(), FormatError> {
    for item in document.items() {
        let Some(token) = significant_tokens(item.syntax()).last() else {
            return Err(FormatError::Internal(
                "document item has no significant token".to_string(),
            ));
        };
        let mut id = token_id(&token, stream)?;
        if stream
            .tokens
            .get(id.next().index())
            .is_some_and(|token| token.kind == T![;])
        {
            id = id.next();
        }
        facts[id.index()].item_boundary = Some(ItemBoundary::Document);
    }

    for (kind, boundary) in [
        (SyntaxKind::ModuleItem, ItemBoundary::Module),
        (SyntaxKind::Block, ItemBoundary::Block),
    ] {
        for container in document
            .syntax()
            .descendants()
            .filter(|node| node.kind() == kind)
        {
            for item in container.children() {
                if let Some(token) = significant_tokens(&item).last() {
                    let id = token_id(&token, stream)?;
                    if facts[id.index()].item_boundary.is_none() {
                        facts[id.index()].item_boundary = Some(boundary);
                    }
                }
            }
        }
    }
    Ok(())
}

fn analyze_expression_groups(
    root: &SyntaxNode,
    stream: &TokenStream,
    facts: &mut [TokenFacts],
) -> Result<(), FormatError> {
    for node in root
        .descendants()
        .filter(|node| matches!(node.kind(), SyntaxKind::BinaryExpr | SyntaxKind::UnionType))
        .filter(|node| {
            node.parent()
                .is_none_or(|parent| parent.kind() != node.kind())
        })
    {
        let mut tokens = significant_tokens(&node);
        let first = tokens.next().ok_or_else(|| {
            FormatError::Internal("binary expression has no significant token".to_string())
        })?;
        let last = tokens.last().unwrap_or_else(|| first.clone());
        let start = token_id(&first, stream)?;
        let end = token_id(&last, stream)?;
        facts[start.index()].group_end = Some(
            facts[start.index()]
                .group_end
                .map_or(end, |current| current.max(end)),
        );

        let mut operator_offsets = Vec::new();
        if node.kind() == SyntaxKind::BinaryExpr {
            collect_binary_operator_offsets(&node, &mut operator_offsets);
        } else {
            collect_union_operator_offsets(&node, &mut operator_offsets);
        }
        facts[start.index()].binary_operators = operator_offsets
            .into_iter()
            .map(|offset| stream.token_id_at_offset(offset))
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(())
}

fn direct_or_descendant_token(
    node: &SyntaxNode,
    kind: SyntaxKind,
    stream: &TokenStream,
) -> Result<Option<TokenId>, FormatError> {
    significant_tokens(node)
        .find(|token| token.kind() == kind)
        .map(|token| token_id(&token, stream))
        .transpose()
}

fn token_id(token: &rue_parser::SyntaxToken, stream: &TokenStream) -> Result<TokenId, FormatError> {
    stream.token_id_at_offset(usize::from(token.text_range().start()))
}

fn collect_binary_operator_offsets(node: &SyntaxNode, operators: &mut Vec<usize>) {
    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(child) if child.kind() == SyntaxKind::BinaryExpr => {
                collect_binary_operator_offsets(&child, operators);
            }
            rowan::NodeOrToken::Token(token) if SyntaxKind::BINARY_OPS.contains(&token.kind()) => {
                operators.push(usize::from(token.text_range().start()));
            }
            _ => {}
        }
    }
}

fn collect_union_operator_offsets(node: &SyntaxNode, operators: &mut Vec<usize>) {
    for element in node.children_with_tokens() {
        match element {
            rowan::NodeOrToken::Node(child) if child.kind() == SyntaxKind::UnionType => {
                collect_union_operator_offsets(&child, operators);
            }
            rowan::NodeOrToken::Token(token) if token.kind() == T![|] => {
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

fn is_generic_punctuation(kind: SyntaxKind) -> bool {
    matches!(kind, T![<] | T![>] | T![,])
}

fn delimiters_match(open: SyntaxKind, close: SyntaxKind) -> bool {
    matches!(
        (open, close),
        (T!['('], T![')']) | (T!['['], T![']']) | (T!['{'], T!['}'])
    )
}
