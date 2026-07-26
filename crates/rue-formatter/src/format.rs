use rue_ast::AstDocument;

use crate::{
    FormatError, analysis::Layout, document::Doc, emit::Formatter, token_stream::TokenStream,
};

pub(crate) fn format_document(
    document: &AstDocument,
    stream: &TokenStream,
) -> Result<Doc, FormatError> {
    let layout = Layout::new(document, stream)?;
    Formatter::new(stream, layout).format()
}
