use crate::token_stream::{CommentPlacement, Gap};

#[derive(Debug, Clone, Default)]
pub(crate) struct Trivia {
    pub(crate) gap: Gap,
}

impl Trivia {
    pub(crate) fn new(gap: Gap) -> Self {
        Self { gap }
    }

    pub(crate) fn dangling(mut gap: Gap) -> Self {
        for comment in &mut gap.comments {
            comment.placement = CommentPlacement::Dangling;
        }
        Self { gap }
    }
}

/// Splits trivia between two movable units without guessing that the whole gap
/// belongs to the unit on its right. Inline comments form the trailing prefix;
/// comments beginning on their own line form the leading suffix.
pub(crate) fn split_between(gap: &Gap) -> (Trivia, Trivia) {
    let split = gap
        .comments
        .iter()
        .position(|comment| comment.placement != CommentPlacement::Trailing)
        .unwrap_or(gap.comments.len());

    let mut trailing = Gap {
        comments: gap.comments[..split].to_vec(),
        newlines: 0,
    };
    let mut leading = Gap {
        comments: gap.comments[split..].to_vec(),
        newlines: gap.newlines,
    };

    if trailing.comments.is_empty() {
        return (Trivia::default(), Trivia::new(leading));
    }
    if leading.comments.is_empty() {
        trailing.newlines = gap.newlines;
        return (Trivia::new(trailing), Trivia::default());
    }

    let first_leading = leading
        .comments
        .first_mut()
        .expect("leading trivia is known to contain a comment");
    trailing.newlines = first_leading.newlines_before;
    first_leading.newlines_before = 0;
    (Trivia::new(trailing), Trivia::new(leading))
}

/// Separates comments that are visually a file banner from documentation
/// attached to the first item. The closest contiguous comment block is leading
/// trivia; earlier blocks remain anchored at the file head.
pub(crate) fn split_file_header(gap: &Gap) -> (Trivia, Trivia) {
    let (_, item_leading) = split_between(gap);
    let gap = &item_leading.gap;
    if gap.comments.is_empty() || gap.newlines > 1 {
        return (Trivia::new(gap.clone()), Trivia::default());
    }

    let mut attached_start = gap.comments.len() - 1;
    while attached_start > 0 && gap.comments[attached_start].newlines_before <= 1 {
        attached_start -= 1;
    }

    let mut header = Gap {
        comments: gap.comments[..attached_start].to_vec(),
        newlines: 0,
    };
    let mut leading = Gap {
        comments: gap.comments[attached_start..].to_vec(),
        newlines: gap.newlines,
    };
    if let Some(first) = leading.comments.first_mut() {
        header.newlines = first.newlines_before;
        first.newlines_before = 0;
    }
    (Trivia::new(header), Trivia::new(leading))
}
