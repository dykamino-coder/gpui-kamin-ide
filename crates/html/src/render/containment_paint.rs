//! Containment contexts keep positioned descendant paint within their own box.
use crate::computed::Computed;
use gpui::{AnyElement, IntoElement, PaintCollect};

pub(super) fn collect(mut children: Vec<AnyElement>, style: &Computed) -> Vec<AnyElement> {
    // CSS Containment 2 §§3.2/3.3 and CSS2 Appendix E: a containment
    // stacking context paints atomically. Flushing under the owner's clip
    // prevents PaintLast descendants from escaping to the document collector.
    // CSS Will Change §2.1 requires the same context for an announced contain.
    if style.contain_layout == Some(true)
        || style.contain_paint == Some(true)
        || style.will_change & crate::computed::wc::STACK != 0
    {
        let (open, close) = PaintCollect::pair();
        children.insert(0, open.into_any_element());
        children.push(close.into_any_element());
    }
    children
}
