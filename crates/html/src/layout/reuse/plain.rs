//! Инертное поддерево: обычный поток текста и блоков без собственных слоёв.
//!
//! Positioned and floating descendants, deferred paint, and interactive or
//! replaced elements can affect state outside a skipped block.

use crate::dom::{Element, Node};
use crate::style::computed::Position;

/// A skip record may use another counter origin only when none of its
/// paint keys or anchors escape the subtree. Both recorders use this result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CounterOrigin {
    Fixed,
    Relative,
}

pub(super) fn skip_origin(e: &Element, vcx: crate::values::ValueCx<'_>) -> Option<CounterOrigin> {
    let mut origin = CounterOrigin::Relative;
    all_elements(e, |node| {
        if origin == CounterOrigin::Relative && super::content::inert_node(node, vcx) {
            return hover_free(node);
        }
        origin = CounterOrigin::Fixed;
        inert(node) && hover_free(node)
    })
    .then_some(origin)
}

/// Элемент `e` сам инертен: обычный поток или таблица, без собственного слоя.
fn inert(e: &Element) -> bool {
    flow_kind(e) && unlayered(e)
}

/// No position, float, transform, opacity or blending: no layer of its own.
fn unlayered(e: &Element) -> bool {
    let style = e.style.used();
    matches!(
        style.reset_positioned.position,
        None | Some(Position::Static)
    ) && style.reset_positioned.z_index.is_none()
        && style.reset_box_layout.float.is_none_or(|f| f == 0)
        && style.reset_transforms.transform.is_none()
        && style.reset_transforms.translate.is_none()
        && style.reset_effects.opacity.is_none_or(|o| o >= 1.0)
        && style.reset_effects.blend.is_none_or(|b| b == 0)
}

/// The element's box is made by its computed style and its subtree alone
/// (css-display-3 §2), and the state it shows lives in the DOM and style,
/// whose changes publish the box's epoch. So these do not qualify: a replaced
/// element (css-display-3 §2.4: its content is an outside resource or a nested
/// document, whose loading publishes no epoch) and a natively drawn control
/// (HTML §15.5); MathML and ruby with their own layout (a ruby annotation may
/// overhang its box, css-ruby-1 §4.4); animations (clock state); first-line
/// and first-letter (taken from an ancestor's lines); a
/// list item's marker (its number comes from siblings). The tag alone decides
/// nothing: `<a>`, `<li>` without a marker or `<dd>` are flow boxes like `<div>`.
pub(super) fn flow_kind(e: &Element) -> bool {
    !embedded(e) && own_state_free(e)
}

/// `flow_kind` but for replaced elements and controls.
fn own_state_free(e: &Element) -> bool {
    !math_layout(e)
        && crate::text::ruby::ruby_role(e).is_none()
        && e.anim.is_none()
        && e.first_letter.is_none()
        && e.first_line.is_none()
        && e.list_item.is_none()
}

/// No `:hover` style of `e`'s own (selectors-4 §10.2). A replay record notes
/// the hover its boxes paint (`replay::Ranges::hovers`), so `flow_kind` admits
/// them; records without that note (skip, far, kept paragraph) require this.
pub(super) fn hover_free(e: &Element) -> bool {
    e.hover.is_none()
}

/// Поддерево `e` можно повторить на экране (`replay`): коробки потока без
/// единиц области просмотра, вертикального письма, гибких и сеточных коробок
/// (их сборка читает размеры окна напрямую, а не через ограничения, чьи
/// чтения записываются). Флоаты — только внутри корня своего контекста
/// форматирования в поддереве (`escaping_floats`). Позиционирование и свои
/// слои — только оставаясь внутри поддерева (`layered::contained`).
pub(super) fn replayable(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    let node = |e: &Element| flow_kind(e) && layout_reads_outside_nothing(e, vcx);
    all_elements(e, node) && !escaping_floats(e) && super::layered::contained(e)
}

fn floating(e: &Element) -> bool {
    e.style
        .used()
        .reset_box_layout
        .float
        .is_some_and(|f| f != 0)
}

/// В поддереве `e` есть флоат, который выходит за его край: сам `e` или
/// флоат, у которого между ним и `e` (включая `e`) нет коробки со своим
/// контекстом форматирования. Флоат внутри такой коробки остаётся в ней: ни
/// строки, ни блоки снаружи его не обходят, а коробка вмещает его по высоте
/// (CSS 2.1 §9.4.1, §10.6.7; css-multicol-1 §2: многоколоночник — корень
/// контекста). Так Blink держит exclusion space внутри результата корня BFC
/// (`LayoutResult`, layout_box_hot.cc:152), а снаружи его не видно.
pub(super) fn escaping_floats(e: &Element) -> bool {
    use crate::layout::box_kind::own_context;
    if floating(e) {
        return true;
    }
    if own_context(e) {
        return false;
    }
    // As `all_elements`: the children of `e` in place, a level per open
    // descendant, so a block of text and empty boxes allocates nothing.
    let mut top = e.children.iter();
    let mut open: Vec<std::slice::Iter<'_, Node>> = Vec::new();
    loop {
        let next = match open.last_mut() {
            Some(level) => level.next(),
            None => top.next(),
        };
        match next {
            Some(Node::Element(c)) => {
                if floating(c) {
                    return true;
                }
                if !c.children.is_empty() && !own_context(c) {
                    open.push(c.children.iter());
                }
            }
            Some(_) => {}
            None => {
                if open.pop().is_none() {
                    return false;
                }
            }
        }
    }
}

/// Элемент `e` сам по себе не мешает повтору поддерева (`replayable`), а
/// в далёком блоке — и без позиционирования и слоёв (`far`).
pub(super) fn replayable_node(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    inert(e) && layout_reads_outside_nothing(e, vcx)
}

/// Без вертикального письма, единиц области просмотра, гибких и сеточных коробок.
fn layout_reads_outside_nothing(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    use crate::style::computed::Display;
    let style = e.style.used();
    !style.wm().vertical()
        && !style.has_viewport_lengths(vcx)
        && !matches!(
            style.reset_box_layout.display,
            Some(Display::Flex | Display::InlineFlex | Display::Grid | Display::InlineGrid)
        )
}

/// The element's content does not come from its subtree: a replaced element or
/// a native control (`<br>` is only a forced line break, css-text-3 §5.1).
fn embedded(e: &Element) -> bool {
    let tag = e.tag.as_str();
    (crate::layout::box_kind::replaced_inline(tag) && tag != "br")
        || crate::layout::box_kind::native_control(tag)
}

/// A MathML element laid out by the math layout (`layout::mathml`).
fn math_layout(e: &Element) -> bool {
    let math = &e.style.used().reset_svg_math;
    math.mathml && math.math_layout.is_some()
}

/// A table box or a box of a table's internal display, laid out by the
/// table (`far::replayable` admits none).
pub(super) fn table(e: &Element) -> bool {
    use crate::style::computed::Display;
    e.tag == "table"
        || matches!(
            e.style.used().reset_box_layout.display,
            Some(
                Display::Table
                    | Display::InlineTable
                    | Display::TableRowGroup
                    | Display::TableRow
                    | Display::TableCell
            )
        )
}

/// Блок `e` заполняет доступную ширину (CSS 2.1 §10.3.3): `width: auto` без
/// `max-width`, не таблица (её авто-ширина — css-tables-3 §3.9.4). Его
/// ширина меняется с каждой шириной потока, повтор там не встанет на место.
pub(super) fn fills_width(e: &Element) -> bool {
    use crate::style::computed::Display;
    let layout = &e.style.used().reset_box_layout;
    let table = e.tag.as_str() == "table"
        || matches!(layout.display, Some(Display::Table | Display::InlineTable));
    !table
        && layout
            .width
            .as_ref()
            .is_none_or(|w| matches!(w, crate::style::values::value::Len::Auto))
        && layout.max_width.is_none()
}

/// Every element of the subtree of `e`, in document order, passes `f`; the
/// walk stops at the first that does not. The stack holds one child iterator
/// per open level, so a block of thousands of inline children costs a few
/// words per level of depth instead of a list of every pending sibling. The
/// children of `e` itself are walked in place and a childless element opens
/// no level: a block of text and empty elements allocates nothing.
pub(super) fn all_elements(e: &Element, mut f: impl FnMut(&Element) -> bool) -> bool {
    if !f(e) {
        return false;
    }
    let mut top = e.children.iter();
    let mut open: Vec<std::slice::Iter<'_, Node>> = Vec::new();
    loop {
        let next = match open.last_mut() {
            Some(level) => level.next(),
            None => top.next(),
        };
        match next {
            Some(Node::Element(child)) => {
                if !f(child) {
                    return false;
                }
                if !child.children.is_empty() {
                    open.push(child.children.iter());
                }
            }
            Some(_) => {}
            None => {
                if open.pop().is_none() {
                    return true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;

mod edges;
mod image;
mod reason;
pub(super) use edges::edges_free;
pub(super) use image::store_image;
pub(super) use reason::not_inert;
