//! Подготовка независимой печатной копии без предварительной оценки геометрии.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::fragment::flow::{copy_flow, empty_floats, exports_floats, regrow_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::BoxStyle;

pub(super) struct Group {
    pub(super) nodes: Vec<Node>,
    pub(super) margins: (f32, f32, f32),
    pub(super) blocks: bool,
    pub(super) float_extent: bool,
}

/// Вертикальная физическая копия; логическое дерево нужно только для полей
/// и рекордера, но не заменяет настоящее дерево в native layout.
pub(super) fn vertical_group(
    nodes: &[Node],
    root: &BoxStyle,
    inline: f32,
    head: f32,
    rl: bool,
    vcx: ValueCx<'_>,
) -> Option<Group> {
    use crate::layout::fragment::flow::{logical_copy, vertical_copy};
    let [Node::Element(e)] = nodes else {
        return None;
    };
    if !vertical_copy(e, rl) {
        return None;
    }
    let e = crate::layout::block::margins::vertical_fragment_box(
        resolved_lengths(e, root, vcx),
        inline,
        rl,
        vcx,
    );
    let logical = logical_copy(&e, rl)?;
    let (mt, mb) = fragment_margins(&logical)?;
    let float_extent = logical
        .children
        .iter()
        .any(|n| matches!(n, Node::Element(e) if exports_floats(e)));
    Some(Group {
        nodes: vec![Node::Element(e)],
        margins: if head > 0.0 {
            (0.0, mb, 0.0)
        } else {
            (mt, mb, mt)
        },
        blocks: true,
        float_extent,
    })
}

pub(super) fn group(
    nodes: &[Node],
    root: &BoxStyle,
    width: f32,
    top: f32,
    breaks: (bool, bool),
    vcx: ValueCx<'_>,
) -> Option<Group> {
    // Общий пробег обтекания — одна измеряемая копия, как у колонок.
    // Одиночный float сохраняет собственный корень и независимый поток.
    let mut flow = if nodes.len() > 1 {
        copy_flow(nodes)
    } else {
        std::borrow::Cow::Borrowed(nodes)
    };
    if let std::borrow::Cow::Owned(nodes) = &mut flow
        && let [Node::Element(root)] = nodes.as_mut_slice()
    {
        root.style = crate::style::cascade::inherit::fragment_group_style(breaks);
    }
    let nodes = flow.as_ref();
    let [Node::Element(e)] = nodes else {
        return None;
    };
    let floated = e.style.used().float.is_some_and(|f| f != 0);
    let float_extent = floated
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(e) if exports_floats(e)));
    let eligible = if floated {
        crate::layout::fragment::flow::page_copy(e, true)
    } else {
        regrow_copy(e) || empty_floats(e) || super::leaf::eligible(e)
    };
    if !eligible {
        return None;
    }
    let e: Element = fragment_box(resolved_lengths(e, root, vcx), width, vcx);
    let (mt, mb) = fragment_margins(&e)?;
    let blocks = !floated || !crate::layout::fragment::clone::content_monolith_blockified(&e);
    Some(Group {
        nodes: vec![Node::Element(e)],
        blocks,
        float_extent,
        margins: if floated {
            (0.0, 0.0, 0.0)
        } else if top > 0.0 {
            (0.0, mb, 0.0)
        } else {
            (mt, mb, mt)
        },
    })
}

#[cfg(test)]
mod float_runs;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod vertical_tests;
