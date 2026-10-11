//! HTML viewport scroll ownership (CSS Overflow 3 §3.3, Scrollbars 1 §2).
//! Colors come from the root; body overflow propagates through an uncontained root.
use crate::dom::Node;
use crate::style::computed::{Computed, Display, Overflow};
use crate::style::values::value::Len;

pub(super) fn policy(nodes: &[Node]) -> Option<Computed> {
    let root = nodes.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "html" => Some(e),
        _ => None,
    })?;
    root.style.scroll.colors.flatten()?;
    if super::canvas_background::any_containment(root) {
        return None;
    }
    let mut c =
        crate::style::cascade::inherit::inherit_unpainted(&Computed::default(), &root.style);
    c.scroll.colors.flatten()?;
    // This is a computed viewport policy, not another currentColor declaration.
    c.scroll.current = [false; 2];
    c.scroll.inherit_width = false;
    let visible = |v| matches!(v, None | Some(Overflow::Visible));
    if let Some(body) = root.children.iter().find_map(|node| match node {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    }) {
        let body_style = crate::style::cascade::inherit::inherit_unpainted(&c, &body.style);
        c.rtl = body_style.rtl;
        c.vertical = body_style.vertical;
        c.vertical_rl = body_style.vertical_rl;
        if visible(c.overflow_x)
            && visible(c.overflow_y)
            && !super::canvas_background::any_containment(body)
        {
            c.overflow_x = body.style.overflow_x;
            c.overflow_y = body.style.overflow_y;
            c.scroll.forced = body.style.scroll.forced;
        }
    }
    let viewport_axis = |v| match v {
        Some(Overflow::Hidden | Overflow::Clip) => Some(Overflow::Hidden),
        _ => Some(Overflow::Scroll),
    };
    c.overflow_x = viewport_axis(c.overflow_x);
    c.overflow_y = viewport_axis(c.overflow_y);
    Some(c)
}

pub(super) fn wrap(nodes: Vec<Node>, policy: Option<Computed>) -> Vec<Node> {
    let Some(c) = policy else { return nodes };
    fn maximum_id(nodes: &[Node]) -> u64 {
        nodes
            .iter()
            .filter_map(|node| match node {
                Node::Element(e) => Some(e.node_id.max(maximum_id(&e.children))),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }
    let id = maximum_id(&nodes) + 1;
    let mut viewport = crate::layout::table::anon::anon_element("html-viewport", nodes);
    viewport.node_id = id;
    viewport.style = Computed {
        display: Some(Display::Block),
        width: Some(Len::Vw(1.0)),
        height: Some(Len::Vh(1.0)),
        border_box: Some(true),
        overflow_x: c.overflow_x,
        overflow_y: c.overflow_y,
        rtl: c.rtl,
        vertical: c.vertical,
        vertical_rl: c.vertical_rl,
        scroll: c.scroll,
        ..Computed::default()
    };
    vec![Node::Element(viewport)]
}

// Keep document preparation identical for initial, updated, and embedded markup.
pub(super) fn prepare(nodes: Vec<Node>) -> (Vec<Node>, Computed) {
    let viewport = policy(&nodes);
    let (nodes, root) =
        super::unwrap_document(super::mark_canvas_background(super::resolve_logical(
            super::propagate_writing_mode(super::viewport_overflow(nodes)),
        )));
    (wrap(nodes, viewport), root)
}

#[cfg(test)]
mod tests;
