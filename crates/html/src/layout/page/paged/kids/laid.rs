//! Подготовка независимой печатной копии без предварительной оценки геометрии.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::fragment::flow::{empty_floats, independent_copy};
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
    let float_extent = empty_floats(&logical);
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
    vcx: ValueCx<'_>,
) -> Option<Group> {
    let [Node::Element(e)] = nodes else {
        return None;
    };
    let floated = e.style.computed().float.is_some_and(|f| f != 0);
    let float_extent = empty_floats(e);
    let eligible = if floated {
        // TRANSITIONAL(F2-1): Float допускается только монолитом без строчного resume; удалить ограничение в P05 после отдельного продолжения его строк и полей.
        let mut flow = e.clone();
        flow.style.used_mut().float = None;
        crate::layout::fragment::clone::content_monolith_blockified(e) && independent_copy(&flow)
    } else {
        independent_copy(e) || float_extent
    };
    // TRANSITIONAL(F2-1): Рост целой копии декорированного блока меняет рамку и координаты соседей; удалить отказ в P04 после краёв каждого печатного фрагмента.
    if !eligible || decorated_lines(e) {
        return None;
    }
    let e: Element = fragment_box(resolved_lengths(e, root, vcx), width, vcx);
    let (mt, mb) = fragment_margins(&e)?;
    Some(Group {
        nodes: vec![Node::Element(e)],
        blocks: !floated,
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
mod tests;

fn decorated_lines(e: &Element) -> bool {
    use crate::style::values::value::Len;
    fn text(e: &Element) -> bool {
        e.children.iter().any(|n| match n {
            Node::Text(t) => !t.trim().is_empty(),
            Node::Element(k) => text(k),
        })
    }
    let b = e.style.computed().borders();
    let decorated = [
        b.top,
        b.bottom,
        e.style.computed().padding.top,
        e.style.computed().padding.bottom,
    ]
    .iter()
    .any(|l| !matches!(l, None | Some(Len::Px(0.0))));
    (decorated && text(e))
        || e.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if decorated_lines(k)))
}
