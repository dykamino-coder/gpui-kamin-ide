//! Eligibility and margin transport for measured float clearance.

use crate::dom::{Element, Node};
use crate::layout::block::struts::{Strut, solve};
use crate::render::{is_blank, out_of_flow};

pub(super) fn mark_start(host: &mut Element, top_open: bool, preceding: &[Node]) {
    if top_open
        && preceding
            .iter()
            .all(|n| is_blank(n) || matches!(n, Node::Element(e) if out_of_flow(&e.style)))
    {
        host.attrs.push(("adjoining-start".into(), "1".into()));
    }
}

pub(super) fn supported(e: &Element) -> bool {
    // Positioned boxes need their original containing-block adapter; the
    // independently measured host cannot preserve that coordinate system.
    e.style.position.is_none()
}

pub(crate) fn remember_float_margin(e: &mut Element, strut: Option<Strut>, emitted: f32) {
    if e.style.float.is_some_and(|f| f != 0) && e.style.float_margin_offset.is_none() {
        // CSS 2.1 §9.5.1 rule 5: the float starts at the source position after
        // preceding block margins. Self-collapsing boxes can leave that
        // margin pending rather than physically emitted in the flow.
        e.style.float_margin_offset = Some(strut.map_or(0.0, |s| solve(s) - emitted));
    }
}
