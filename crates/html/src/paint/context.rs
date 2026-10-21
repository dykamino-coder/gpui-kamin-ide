//! Состояние обхода paint, общее только для элементов одного дерева отрисовки.

use std::cell::RefCell;

#[derive(Default)]
pub struct PaintCx {
    pub(crate) effect: RefCell<Option<crate::layout::fragment::effect_fragment::Scope>>,
    pub(crate) gap: RefCell<Option<crate::layout::fragment::gap_fragment::Scope>>,
    pub(crate) coordinates: RefCell<Vec<crate::layout::fragment::static_coordinates::Mapping>>,
    pub(crate) outlines:
        RefCell<Vec<Option<Vec<crate::paint::decorations::outline_phase::Outline>>>>,
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
        let owner = Rc::new(PaintCx::default());
        let other = PaintCx::default();
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
