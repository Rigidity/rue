use rue_parser::{SyntaxKind, SyntaxNode};

use crate::FormatError;

pub(crate) fn validate_node_kinds(root: &SyntaxNode) -> Result<(), FormatError> {
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
