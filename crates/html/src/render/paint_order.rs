//! GPUI wrapper that hoists an inline absolute box into the step-8 collector.

use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use gpui::{AnyElement, IntoElement};

/// Строчный абсолют с `z-index: auto | 0` в позднем слое (`late_push`): шаг 8
/// приложения E CSS 2.1 — позиционированные рисуются ПОСЛЕ строчного
/// содержимого (шаг 7) своего контекста наложения. Строки абзаца уходят в
/// собиратель (`PaintInline`) и рисуются в его конце, а поздний слой — прямой
/// ребёнок контейнера и красился раньше них: текст ложился поверх абсолюта
/// (`ch-unit-001`, `ic-unit-001`). `PaintLast` ставит коробку в собиратель по
/// ключу в порядке разметки. Узел вне обхода (порождённый сборщиком) остаётся
/// на прежнем пути.
pub(crate) fn inline_abs_paint_last(e: &Element, el: AnyElement, cx: &LayoutCx) -> AnyElement {
    let u = &*cx.unkeyed;
    let known =
        u.0.get(&e.node_id)
            .map(|&end| u.1.is_none_or(|last| last < end));
    if e.style.z_index.unwrap_or(0) == 0 && known == Some(true) {
        gpui::PaintLast::new(el)
            .key(cx.next_paint_key())
            .into_any_element()
    } else {
        el
    }
}
