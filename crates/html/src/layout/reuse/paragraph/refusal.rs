//! Why a paragraph was built without being kept, and its DevTools counter.

/// Why a paragraph was built without being kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The paragraph's route is not plain: a native request, a line clamp or
    /// a first-line cut of its own.
    Context,
    /// The container has no box in the box tree.
    NoBox,
    /// Built for fragmentation (`key::plain`).
    Fragmented,
    /// Built under a line clamp or for its measure.
    Clamp,
    /// Built as a ruby base or annotation.
    Ruby,
    /// Built with a line trim (`text-box-trim`) of its edges.
    Trim,
    /// Built for the measure of an enclosing inline.
    InlineMeasure,
    /// Built under deferred out-of-flow boxes.
    OutOfFlow,
    /// A top-level element's subtree paints a hover.
    Hover,
    /// A top-level element's subtree is not inert.
    Subtree,
    /// An atom's box depends on the available width (a percentage or an
    /// automatic inline size, percentage edges).
    AtomWidth,
    /// An atom inherits from an inline ancestor or has an inline containing block.
    AtomParent,
    /// An atom's node has no key or no path below the paragraph's nodes.
    AtomPath,
    /// The paragraph holds what a later frame could not lay out without a build.
    Paragraph,
}

/// The number of refusals (`KeepStats::refused`).
pub(crate) const REFUSALS: usize = Refusal::Paragraph as usize + 1;

impl Refusal {
    /// The `why` journal's work counter of this refusal.
    pub(super) fn note(self) {
        match self {
            Refusal::Context => {
                crate::work::add!(ParagraphsRefusedContext, 1);
            }
            Refusal::NoBox => {
                crate::work::add!(ParagraphsRefusedNoBox, 1);
            }
            Refusal::Fragmented => {
                crate::work::add!(ParagraphsRefusedFragmented, 1);
            }
            Refusal::Clamp => {
                crate::work::add!(ParagraphsRefusedClamp, 1);
            }
            Refusal::Ruby => {
                crate::work::add!(ParagraphsRefusedRuby, 1);
            }
            Refusal::Trim => {
                crate::work::add!(ParagraphsRefusedTrim, 1);
            }
            Refusal::InlineMeasure => {
                crate::work::add!(ParagraphsRefusedInlineMeasure, 1);
            }
            Refusal::OutOfFlow => {
                crate::work::add!(ParagraphsRefusedOutOfFlow, 1);
            }
            Refusal::Hover => {
                crate::work::add!(ParagraphsRefusedHover, 1);
            }
            Refusal::Subtree => {
                crate::work::add!(ParagraphsRefusedSubtree, 1);
            }
            Refusal::AtomWidth => {
                crate::work::add!(ParagraphsRefusedAtomWidth, 1);
            }
            Refusal::AtomParent => {
                crate::work::add!(ParagraphsRefusedAtomParent, 1);
            }
            Refusal::AtomPath => {
                crate::work::add!(ParagraphsRefusedAtomPath, 1);
            }
            Refusal::Paragraph => {
                crate::work::add!(ParagraphsRefusedParagraph, 1);
            }
        }
    }
}
