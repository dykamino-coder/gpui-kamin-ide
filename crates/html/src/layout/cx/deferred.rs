//! Отложенная отрисовка: общие владельцы Deferred и paint-order текущего дерева.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct DeferredPaintContext {
    /// Prepaint frame shared only by elements of this render tree.
    /// Traversal state captured by this render tree's deferred painters.
    pub(crate) paint: Rc<Deferred>,
    pub(crate) vertical_paint: Rc<crate::text::vertical::VerticalPaint>,
    /// Paint-order and deferred-geometry boundaries of the box being built.
    pub(crate) depth: crate::paint::effects::paint_scope::Depth,
    /// Paint keys of CSS 2.1 Appendix E step 8 (`gpui::PaintLast`), in build
    /// order: an ancestor gets a smaller key than its descendants. Shared by
    /// everything that paints into one collector, nested documents included.
    pub(crate) keys: Rc<std::cell::Cell<u64>>,
    /// Positions of the document's nodes in a forward walk and of the LAST
    /// positioned node that keeps no paint key (`unkeyed_positions`).
    pub(crate) unkeyed: Rc<crate::layout::order::Unkeyed>,
    /// Nearest gap-decorated container collecting child bounds.
    pub(crate) gap_items: Option<crate::paint::gap_rules::GapItems>,
    /// Keep scrollbar hover and drag at rest for a static host capture.
    pub freeze_scrollbars: bool,
}

pub(crate) type Outline = (gpui::AnyElement, gpui::ContentMask<gpui::Pixels>);

#[derive(Default)]
pub struct Deferred {
    pub(crate) effect: RefCell<Option<crate::layout::fragment::effect_fragment::Scope>>,
    pub(crate) gap: RefCell<Option<crate::layout::fragment::gap_fragment::Scope>>,
    pub(crate) coordinates: RefCell<Vec<crate::layout::fragment::static_coordinates::Mapping>>,
    pub(crate) outlines: RefCell<Vec<Option<Vec<Outline>>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::layout::fragment::{effect_fragment, gap_fragment};
    use gpui::{Bounds, ContentMask, point, px, size};
    use std::rc::Rc;

    #[test]
    fn deferred_owner_keeps_fragment_state_separate_from_other_render_trees() {
        let doc = DocState::new(Default::default());
        let owner = Rc::new(Deferred::default());
        let other = Deferred::default();
        let retained = doc.values().with_paint(Some(&owner)).keep();
        drop(owner);
        let owner = retained.vcx().paint.unwrap();
        let bounds = Bounds {
            origin: point(px(10.0), px(20.0)),
            size: size(px(30.0), px(40.0)),
        };
        let scope = effect_fragment::Scope {
            parent: ContentMask { bounds },
            fragment: bounds,
            clip: bounds,
        };
        effect_fragment::with(owner, Some(scope), || {
            assert!(effect_fragment::active(owner));
            assert!(!effect_fragment::active(&other));
            assert!(gap_fragment::current(&other).is_none());
            effect_fragment::with(&other, Some(scope), || {
                assert!(effect_fragment::active(owner));
                assert!(effect_fragment::active(&other));
            });
            assert!(!effect_fragment::active(&other));
        });
        assert!(!effect_fragment::active(owner));
        owner.outlines.borrow_mut().push(Some(Vec::new()));
        other.outlines.borrow_mut().push(None);
        assert!(matches!(owner.outlines.borrow_mut().pop(), Some(Some(_))));
        assert!(matches!(other.outlines.borrow_mut().pop(), Some(None)));
    }
}
