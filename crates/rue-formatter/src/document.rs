//! A deliberately small, private pretty-printing document model.

#[derive(Debug, Clone)]
pub(crate) enum Doc {
    Nil,
    Text(String),
    Concat(Vec<Self>),
    Line(LineKind),
    Indent(Box<Self>),
    Group(Box<Self>),
    /// A local fill boundary with continuation indentation applied only when
    /// that boundary breaks. Ordinary groups cannot express that conditional
    /// indentation when their surrounding binary chain is already broken.
    Fill {
        doc: Box<Self>,
        indent_on_break: bool,
    },
    IfBreak {
        broken: Box<Self>,
        flat: Box<Self>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineKind {
    Soft,
    Hard,
    Empty,
}

impl Doc {
    pub(crate) fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    pub(crate) fn concat(docs: impl IntoIterator<Item = Self>) -> Self {
        let mut flattened = Vec::new();
        for doc in docs {
            match doc {
                Self::Nil => {}
                Self::Concat(children) => flattened.extend(children),
                doc => flattened.push(doc),
            }
        }
        Self::Concat(flattened)
    }

    pub(crate) fn space() -> Self {
        Self::text(" ")
    }

    pub(crate) fn soft_line() -> Self {
        Self::Line(LineKind::Soft)
    }

    pub(crate) fn fill(doc: Self, indent_on_break: bool) -> Self {
        Self::Fill {
            doc: Box::new(doc),
            indent_on_break,
        }
    }

    pub(crate) fn hard_line() -> Self {
        Self::Line(LineKind::Hard)
    }

    pub(crate) fn empty_line() -> Self {
        Self::Line(LineKind::Empty)
    }

    pub(crate) fn indent(self) -> Self {
        Self::Indent(Box::new(self))
    }

    pub(crate) fn group(self) -> Self {
        Self::Group(Box::new(self))
    }

    pub(crate) fn if_break(broken: Self, flat: Self) -> Self {
        Self::IfBreak {
            broken: Box::new(broken),
            flat: Box::new(flat),
        }
    }
}
