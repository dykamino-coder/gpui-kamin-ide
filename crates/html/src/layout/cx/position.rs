//! Containing blocks и якоря: входы positioned layout и проб OOF текущего кадра.

use crate::layout::positioned::containing_block::Positioning;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct PositioningContext {
    /// Containing-block layers of the current build pass (`Positioning`):
    /// a new document (`render`, a nested document) starts its own.
    pub(crate) state: Rc<Positioning>,
    /// Идентичности отложенного поддерева сохраняют порядок исходного дерева.
    pub(crate) oof_identities:
        Option<Rc<crate::layout::positioned::anchor::identities::Identities>>,
    /// Входы размера корня OOF из геометрии текущего кадра.
    pub(crate) oof_sizes: Option<(u64, crate::layout::positioned::anchor::resolve::SizeInputs)>,
    /// Проба position-try: только корень публикует свой layout id, без prepaint.
    pub(crate) oof_trial: Option<(u64, Rc<crate::layout::positioned::anchor::trials::Probe>)>,
    /// Line-box hand-off for the static-position probe of the inline atom
    /// being built (`LineBoxSlot`); `None` outside such an atom.
    pub(crate) line_box: Option<crate::layout::positioned::containing_block::LineBoxSlot>,
    /// Offer of a positioned inline ancestor as the containing block of the
    /// inline atom being built (`InlineCbOffer`).
    pub(crate) inline_cb: Option<std::rc::Rc<crate::text::inline::InlineCbOffer>>,
}
