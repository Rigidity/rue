use rue_ast::{AstDocument, AstNode};

use crate::{
    FormatError, analysis::Layout, document::Doc, emit::Formatter, syntax::validate_node_kinds,
    token_stream::TokenStream,
};

pub(crate) fn format_document(
    document: &AstDocument,
    stream: &TokenStream,
) -> Result<Doc, FormatError> {
    validate_node_kinds(document.syntax())?;
    let layout = Layout::new(document, stream)?;
    Formatter::new(stream, layout).format()
}
