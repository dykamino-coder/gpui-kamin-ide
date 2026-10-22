//! Подготовка независимой печатной копии без предварительной оценки геометрии.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::fragment::flow::independent_copy;
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::Computed;

pub(super) struct Group {
    pub(super) nodes: Vec<Node>,
    pub(super) margins: (f32, f32, f32),
}

pub(super) fn group(
    nodes: &[Node],
    root: &Computed,
    width: f32,
    top: f32,
    vcx: ValueCx<'_>,
) -> Option<Group> {
    let [Node::Element(e)] = nodes else {
        return None;
    };
    if !independent_copy(e) {
        return None;
    }
    let e: Element = fragment_box(resolved_lengths(e, root, vcx), width, vcx);
    let (mt, mb) = fragment_margins(&e)?;
    Some(Group {
        nodes: vec![Node::Element(e)],
        margins: if top > 0.0 {
            (0.0, mb, 0.0)
        } else {
            (mt, mb, mt)
        },
    })
}
