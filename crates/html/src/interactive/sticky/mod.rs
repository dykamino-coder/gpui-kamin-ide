//! Sticky boxes (`position: sticky`): a probe that records the parent's box
//! and the visible part of the scrollport, and the `Sticky` wrapper with the
//! thresholds from `inset`.
//!
//! A sticky box paints as a positioned box of step 8 of its stacking context,
//! above the flow scrolling under it; GPUI's deferred pass is kept only for a
//! positive `z-index` (css-position-3 §stickypos).

use crate::hooks::IntoHooked;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Styled};

pub mod element;

/// Распорка, снимающая коробку родителя и видимую часть ленты.
///
/// Абсолютная и во весь родитель: так её собственный прямоугольник и есть
/// коробка родителя, а обрезка на замере — это видимая часть прокрутки.
pub(crate) fn sticky_probe(frame: crate::interactive::sticky::element::StickyCell) -> AnyElement {
    gpui::canvas(
        move |bounds, window, _| {
            frame.set(crate::interactive::sticky::element::StickyFrame {
                container: Some(bounds),
                // Under a scroll node the content mask is its interest
                // rect (what a scrolled replay may show), not the scrollport.
                viewport: Some(window.scroll_visible_mask()),
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Обернуть элемент липкой рамкой, если `position: sticky`.
pub(crate) fn sticky_wrap(
    el: AnyElement,
    c: &Computed,
    frame: &crate::interactive::sticky::element::StickyCell,
    allowed: bool,
    // The box's link in the shift chain its subtree was built with
    // (`ScrollContext::sticky`).
    link: Option<crate::interactive::scroll_extent::StickyShift>,
) -> AnyElement {
    if c.reset_positioned.position != Some(crate::style::computed::Position::Sticky) {
        return el;
    }
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Auto) | None => None,
        // Проценты у порога считаются от видимой части; её размер известен
        // только на отрисовке, поэтому берём ноль — как `top: 0`.
        Some(_) => Some(0.0),
    };
    let link = link.unwrap_or_else(|| crate::interactive::scroll_extent::ShiftLink::inside(None));
    let mut wrapper = crate::interactive::sticky::element::Sticky::new(el, frame.clone(), link);
    wrapper.top = side(c.reset_positioned.inset.top);
    wrapper.bottom = side(c.reset_positioned.inset.bottom);
    wrapper.left = side(c.reset_positioned.inset.left);
    wrapper.right = side(c.reset_positioned.inset.right);
    // Внутри отложенного поддерева липкий элемент рисуется на месте:
    // откладывать повторно нельзя. Без единого порога (все четыре `auto`)
    // липкий не сдвигается вовсе — css-position-3 §sticky-pos: «If both
    // inset properties in a given axis are auto, no offsets are added in that
    // axis», — и красится как `relative`, в порядке дерева (CSS 2.1 прил. E,
    // шаг 8). Отложенный, он ложился ПОВЕРХ следующего позиционированного
    // брата (`position-sticky-stacking-context-002`: красная половина
    // липкого вылезала поверх `relative`-брата). Servo такому узлу липкой
    // рамки не заводит (`stacking_context.rs`, ветка всех `auto`).
    let thresholds = wrapper.top.is_some()
        || wrapper.bottom.is_some()
        || wrapper.left.is_some()
        || wrapper.right.is_some();
    // With `z-index: auto | 0` the box is a positioned box of step 8
    // (CSS 2.1 Appendix E): the step-8 collector of its stacking context
    // paints it after the in-flow content scrolling under it, in tree
    // order, inside its scroller (`step8::paints_at_step8`), as Blink's
    // and WebRender's sticky boxes stay in their stacking context. GPUI's
    // deferred pass painted it above everything, outside its scroller and
    // its scroll node: a wheel step then rebuilt the document.
    let z = c.reset_positioned.z_index.unwrap_or(0);
    if !allowed || !thresholds || z <= 0 {
        return wrapper.hooked();
    }
    // Lifted above the flow in paint order only: the box stays under its
    // scroller's node, its follower node follows that scroller
    // (`gpui::Deferred::in_scroll_node`).
    gpui::deferred(wrapper.into_hooked())
        .with_priority(z as usize)
        .in_scroll_node()
        .into_any_element()
}
