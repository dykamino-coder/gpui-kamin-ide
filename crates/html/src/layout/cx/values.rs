//! Документ и viewport: владелец Document, значения живут в аренах документа.

use super::BuildDoc;

#[derive(Clone, Default)]
pub(crate) struct ValueContext {
    /// The document being built: its quirks mode, `@position-try` rules,
    /// medium and the arenas of its `calc()`/`anchor()` values. Set by
    /// `Document::render` and for each nested document.
    pub(crate) doc: BuildDoc,
    /// CSS viewport retained by deferred effects and nested document trees.
    pub(crate) viewport: (f32, f32),
}
