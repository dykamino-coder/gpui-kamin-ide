//! Анонимный бокс viewport: несёт политику прокрутки корня и цвет холста.

use super::{Computed, Display, Node};
use crate::style::values::value::Len;

pub(in crate::document) fn wrap(
    nodes: Vec<Node>,
    policy: Option<Computed>,
    canvas: Option<crate::style::values::value::Color>,
) -> Vec<Node> {
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
    let (rtl, writing_mode) = (c.rtl, c.writing_mode);
    viewport.style = {
        let mut with_inherited = Computed {
            display: Some(Display::Block),
            width: Some(Len::Vw(1.0)),
            height: Some(Len::Vh(1.0)),
            border_box: Some(true),
            background: canvas,
            overflow_x: c.overflow_x,
            overflow_y: c.overflow_y,
            scroll: c.scroll,
            viewport_unit_insets: c.viewport_unit_insets,
            ..Computed::default()
        };
        with_inherited.rtl = rtl;
        with_inherited.writing_mode = writing_mode;
        with_inherited
    }
    .into();
    vec![Node::Element(viewport)]
}
