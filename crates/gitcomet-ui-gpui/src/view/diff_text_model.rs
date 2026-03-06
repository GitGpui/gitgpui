use super::*;

#[derive(Clone, Debug)]
pub(super) struct CachedDiffTextSegment {
    pub(super) text: SharedString,
    pub(super) in_word: bool,
    pub(super) in_query: bool,
    pub(super) syntax: SyntaxTokenKind,
}

#[derive(Clone, Debug)]
pub(super) struct CachedDiffStyledText {
    pub(super) text: SharedString,
    pub(super) highlights: Arc<Vec<(Range<usize>, gpui::HighlightStyle)>>,
    pub(super) highlights_hash: u64,
    pub(super) text_hash: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SyntaxTokenKind {
    None,
    Comment,
    String,
    Keyword,
    Number,
    Function,
    Type,
    Property,
    Constant,
    Punctuation,
}
