//! The roots of the damage of a batch that reaches the box tree or layout,
//! and the document as the last of them.
//!
//! Blink marks the layout object of each element whose `StyleDifference`
//! needs layout or reattachment (`LayoutObject::SetNeedsLayout`,
//! `Element::RebuildLayoutTree`). Here an element whose boxes change
//! (`Damage::Rebox`) or whose layout values change (`Damage::Layout`) is a
//! root: its boxes are built again (`rebox`), and layout runs over the
//! retained tree with them. The executor widens a root up to the element
//! that contains what it reaches; the document is the widest root, whose
//! boxes are those of a build (`fall_back_document`).

use super::levels::Levels;
use crate::paint_path as why;
use crate::style::computed::Damage;
use markup5ever_rcdom::Handle;

/// The elements whose boxes the batch builds again: those whose boxes
/// change and those whose layout values change.
pub(super) fn roots(levels: &Levels) -> Vec<Handle> {
    let layout = levels
        .restyles
        .values()
        .filter(|r| r.damage.0 == Damage::Layout);
    let layout: Vec<Handle> = layout.map(|r| r.handle.clone()).collect();
    why::add(&why::DAMAGE_LAYOUT, layout.len() as u64);
    why::add(&why::DAMAGE_REBOX, levels.deeper.len() as u64);
    let deeper = levels.deeper.iter().map(|d| d.handle.clone());
    deeper.chain(layout).collect()
}

/// The causes of the roots of `levels`, for the journal.
pub(super) fn causes(levels: &Levels) -> String {
    let mut causes: Vec<&str> = levels.deeper.iter().map(|d| d.why).collect();
    causes.extend(
        levels
            .restyles
            .values()
            .filter(|r| r.damage.0 == Damage::Layout)
            .map(|r| r.damage.1),
    );
    causes.sort_unstable();
    causes.dedup();
    causes.join(", ")
}

/// The document is the root of the batch's damage (`cause`): its boxes are
/// built by the document's build.
pub(in crate::document::revision) fn fall_back_document<T>(cause: &str) -> Option<T> {
    why::add(&why::DAMAGE_REBOX, 1);
    why::refuse_with(&why::REFUSED_DOCUMENT, || format!("the document: {cause}"));
    None
}
