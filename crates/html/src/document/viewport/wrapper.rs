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
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::Block),
                    width: Some(Len::Vw(1.0)),
                    height: Some(Len::Vh(1.0)),
                    overflow_x: c.overflow_x,
                    overflow_y: c.overflow_y,
                    scroll: c.scroll,
                    ..Default::default()
                },
            ),
            reset_border: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBorder {
                    border_box: Some(true),
                    ..Default::default()
                },
            ),
            reset_background: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBackground {
                    background: canvas,
                    ..Default::default()
                },
            ),
            reset_positioned: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetPositioned {
                    viewport_unit_insets: c.viewport_unit_insets,
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.rtl = rtl;
        with_inherited.writing_mode = writing_mode;
        with_inherited
    }
    .into();
    vec![Node::Element(viewport)]
}
