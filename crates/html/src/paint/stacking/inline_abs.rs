//! Step-8 wrapper of an inline absolute box: the layout order decision picks
//! the box, this adapter wraps it in `PaintLast` with the next paint key.

use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use crate::layout::order::hoists_inline_abs;
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
    if hoists_inline_abs(e, cx) {
        gpui::PaintLast::new(el)
            .key(cx.next_paint_key())
            .into_any_element()
    } else {
        el
    }
}
