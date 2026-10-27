//! Атомарная покраска контекста наложения от `contain: layout|paint`, `will-change` и
//! `transform-style: preserve-3d` (CSS Containment 2 §3.2/3.3, CSS Will Change §2.1, CSS 2
//! Приложение E): дети оборачиваются парой `PaintCollect`, чтобы отложенные позиционированные
//! потомки сбрасывались под обрезкой владельца, а не уходили в сборщик документа.
use crate::paint::coordinator::PaintCollect;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement};

pub(crate) fn collect(mut children: Vec<AnyElement>, style: &Computed) -> Vec<AnyElement> {
    // CSS Containment 2 §§3.2/3.3 and CSS2 Appendix E: a containment
    // stacking context paints atomically. Flushing under the owner's clip
    // prevents PaintLast descendants from escaping to the document collector.
    // CSS Will Change §2.1 requires the same context for an announced contain.
    if style.contain_layout == Some(true)
        || style.contain_paint == Some(true)
        || style.will_change & crate::style::computed::wc::STACK != 0
        // css-transforms-2 §transform-style-property: `preserve-3d` is a
        // stacking context too; its positioned descendants sort by depth with
        // the context's planes (`transform-style-stacking-context`).
        || style.preserve_3d == Some(true)
    {
        let (open, close) = PaintCollect::pair();
        children.insert(0, open.into_any_element());
        children.push(close.into_any_element());
    }
    children
}
