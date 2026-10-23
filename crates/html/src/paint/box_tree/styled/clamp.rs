//! Оформление коробки по неизменяемому плану текущей раскладки.
use crate::layout::{multicol::spanner::multicol_container, replaced::limits::auto_clamp_limit};
use crate::style::values::value::{px_lossy_zero, px_strict};
use crate::{dom::Element, layout::cx::LayoutCx, style::computed::Computed};
use gpui::{Styled, px};
pub(super) fn apply(mut d: gpui::Div, e: &Element, c: &Computed, cx: &LayoutCx) -> gpui::Div {
    let sized = e.style.height.is_some() || e.style.min_height.is_some();
    // A box fragmented at a line-clamp point ends there (css-overflow-4
    // §5.3): content after the point no longer sizes it (`line-clamp-024/
    // 025`: margins of boxes after the point).
    if !sized && let Some(h) = cx.clamp_box_height(e.node_id) {
        d = d.max_h(px(h));
    }
    // В многоколоночнике `continue: collapse` ведёт себя как `auto`
    // (css-overflow-4 §5.2): срез не действует (`line-clamp-039`).
    let measuring = cx.clamp_plan(e.node_id).is_none();
    let multicol = multicol_container(&e.style);
    // Срез меряется от верха поля содержимого, а `max_h` раскладки —
    // высота рамочной коробки
    // (`paint/apply/`: движок всегда трактует размер как border-box). Без
    // вертикальных паддингов и рамок самого контейнера коробка выходила ниже
    // на их сумму и резала последнюю строку (`line-clamp-auto-002`: 128
    // вместо 1+4+128+4+1 = 138).
    let side = px_lossy_zero;
    let bw = c.borders();
    let mbp_y = side(bw.top) + side(bw.bottom) + side(c.padding.top) + side(c.padding.bottom);
    if !multicol && !measuring && e.style.clamp_by_height() && auto_clamp_limit(c).is_some() {
        if let Some(cut) = cx
            .clamp_plan(e.node_id)
            .map(|p| p.cut)
            .filter(|c| !sized && c.is_finite())
        {
            d = d.max_h(px(cut + mbp_y));
            crate::text::clamp::extend_clip(d.style(), e.node_id, cut, side(c.padding.bottom), cx);
        }
        // Прячется только содержимое ЗА точкой среза — по блочной оси;
        // оставленные строки по строчной оси переполняют коробку как
        // обычно (css-overflow-4 §5.3: `overflow` клампом не меняется;
        // `block-ellipsis-037`: непереносимое слово шире коробки).
        d.style().overflow.y = Some(gpui::Overflow::Hidden);
    }
    if let Some((n, cut)) = e
        .style
        .clamp_lines()
        .filter(|_| !multicol)
        .and_then(|n| cx.clamp_plan(e.node_id).map(|p| (n, p.cut)))
    {
        // Без `Styled::line_clamp`: тот попутно включает `overflow_hidden`,
        // а что прятать, решает срез ниже.
        d.text_style().line_clamp = Some(n as usize);
        // The clamp point only replaces the AUTOMATIC height: a tighter
        // `max-height` still sizes the box and the kept lines overflow it
        // (css-overflow-4 §5.3; `webkit-line-clamp-with-max-height`). The
        // box is border-box here, the style value content-box unless
        // `box-sizing: border-box`.
        let style_max = px_strict(c.max_height);
        let style_max = style_max.map(|m| {
            if c.border_box == Some(true) {
                m
            } else {
                m + mbp_y
            }
        });
        let tighter = style_max.filter(|m| cut.is_finite() && *m < cut + mbp_y);
        if !sized && cut.is_finite() && tighter.is_none() {
            d = d.max_h(px(cut + mbp_y));
        }
        // Счётный кламп прячет только то, что ЗА точкой среза; сами
        // оставленные строки переполняют коробку как обычно (css-overflow-4
        // §5.3 — `overflow` клампом не меняется). «Резать нечего»
        // (бесконечная точка: за N-й строкой своего абзаца ничего нет, хвост
        // абзаца уже снял бюджет строк) — обрезки нет, и «Line 4…» видна под
        // коробкой с `height: 3lh` (`line-clamp-011/035`).
        if cut.is_finite() {
            d.style().overflow.y = Some(gpui::Overflow::Hidden);
            // Содержимое ЗА точкой среза не видно и в нижнем паддинге
            // контейнера (css-overflow-4 §5.3: оно «visually hidden», а не
            // обрезано краем паддинга; Blink — `is_hidden_for_paint`):
            // нижний край обрезки — край поля содержимого
            // (`webkit-line-clamp-050`: «Line4» в паддинге 10px).
            let pad_bottom = side(c.padding.bottom);
            if pad_bottom > 0.0 && d.style().overflow_clip_offset.is_none() {
                d.style().overflow_clip_offset = Some([0.0, 0.0, -pad_bottom, 0.0]);
            }
            if !sized {
                crate::text::clamp::extend_clip(d.style(), e.node_id, cut, pad_bottom, cx);
            }
            // Lines before the clamp point overflowing a tighter
            // `max-height` stay visible: the clip edge is the clamp point.
            if let Some(m) = tighter.filter(|_| !sized) {
                let reach = cut + mbp_y - pad_bottom - m;
                let clip = d.style().overflow_clip_offset.get_or_insert([0.0; 4]);
                clip[2] = clip[2].max(reach);
            }
        }
    }
    d
}
