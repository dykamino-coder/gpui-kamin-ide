//! Whether a block child paints at step 8 of CSS 2.1 Appendix E (`PaintLast`).

use crate::layout::cx::LayoutCx;
use crate::layout::order::paint_last_ok;
use crate::paint::stacking::z_index_applies;
use crate::style::computed::BoxStyle;

/// Block child `e` paints at step 8, after the blocks and lines of its flow.
/// `rest` are the nodes after `e` in its container: the answer depends on
/// them (`paint_last_ok`), so a replay of the block asks it again
/// (`reuse::FlowCx::replay`).
pub(crate) fn paints_at_step8(
    e: &crate::dom::Element,
    rest: &[crate::dom::Node],
    inherited: &BoxStyle,
    under_tf: bool,
    cx: &LayoutCx,
) -> bool {
    // Позиционированный блок с `z-index: auto | 0` рисуется на шаге 8
    // приложения E CSS 2.1 — ПОСЛЕ блоков и строк потока, — а у нас
    // порядок краски был порядком детей: следующий блок закрашивал
    // сдвинутый `relative` (`position-relative-035`) и абсолют с одной
    // свободной осью (`right-offset-003`). `PaintLast` меняет только
    // краску — раскладка и место в потоке те же, поэтому строки он не
    // рвёт (запись про распорку выше): внутри строки элементы идут
    // через `pending`, а не сюда. Положительный `z-index` и оконный `fixed`
    // уже отложены `layered`, отрицательный — подложка выше. Под трансформом
    // `fixed` остаётся в контексте предка и требует того же шага 8, что
    // абсолют: перестановка детей не поднимает его над отложенными строками.
    let step8 = (matches!(
        e.style.used().reset_positioned.position,
        Some(crate::style::computed::Position::Relative)
            | Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Sticky)
    ) || (under_tf
        && e.style.used().reset_positioned.position
            == Some(crate::style::computed::Position::Fixed)))
        && e.style.used().reset_positioned.z_index.unwrap_or(0) == 0
        && !matches!(e.tag.as_str(), "html" | "body")
        && paint_last_ok(e, rest, cx);
    // Непозиционированный элемент с `opacity` < 1 красится на том же слое, что
    // позиционированные с `z-index: 0` (css-color-4 §opacity: «painted
    // on the same layer … as positioned elements with stacking order
    // 0»; Blink кладёт такой слой в список z-порядка с нулём): после
    // блоков и строк потока, в порядке разметки (`t32-opacity-zorder-c`).
    step8
        || (e.style.used().reset_positioned.position.is_none_or(|p| p == crate::style::computed::Position::Static)
            // `z-index` у непозиционированного не действует
            // (CSS 2.1 §9.9.1 «Applies to: positioned elements»).
            && (e.style.used().reset_positioned.z_index.unwrap_or(0) == 0
                || !z_index_applies(&e.style.used(), inherited))
            // Только прозрачность: у `contain`/`will-change`/
            // `transform` положительный `z-index` потомков держится
            // на краске на месте (`contain-paint-stacking-context-*`).
            && (e.style.used().reset_effects.opacity.is_some_and(|o| o < 1.0)
                // css-transforms-1 §transform-rendering: a transformed
                // box establishes a stacking context and is painted
                // as a positioned `z-index: 0` layer (Blink puts it in
                // the z-order list with 0): `perspective-zero` — a
                // static transformed box after a `relative` one.
                || e.style.used().reset_transforms.transform.is_some()
                || e.style.used().reset_transforms.translate.is_some()
                // Compositing 1 §mix-blend-mode: paint the new context at level zero.
                || e.style.used().reset_effects.blend.is_some_and(|b| b != 0))
            && !e.style.used().reset_positioned.z_index.is_some_and(|z| z > 0 && z_index_applies(&e.style.used(), inherited))
            && !matches!(e.tag.as_str(), "html" | "body")
            && paint_last_ok(e, rest, cx))
}
