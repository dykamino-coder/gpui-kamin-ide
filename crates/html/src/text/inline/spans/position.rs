//! Смещения кусков по байтовым отрезкам текста абзаца: относительный сдвиг (CSS 2.1 §9.4.3,
//! на поток не влияет) и сдвиг базовой линии `vertical-align` (§10.8.1: `sub`/`super`, длина,
//! процент от `line-height` куска, `text-top`/`text-bottom`, `middle`) с учётом центральной
//! доминантной базовой линии повёрнутой строки (css-writing-modes-4 §4.2, css-inline-3);
//! к вне-поточной коробке выравнивание не применяется.

use super::{text_ranges, text_size};
use crate::layout::options::RenderOpts;
use crate::style::values::value::Len;
use crate::text::inline::*;

/// Относительный сдвиг ПО КУСКАМ: отрезок байт → смещение отрисовки.
///
/// Отдельно от `shift_spans`: тот растит строчную коробку, а относительный
/// сдвиг на поток не влияет вовсе (CSS 2.1 §9.4.3).
// Each relative span retains its text range and two-axis displacements.
#[allow(clippy::type_complexity)]
pub fn rel_spans(pieces: &[Piece]) -> Vec<(std::ops::Range<usize>, (f32, f32), (f32, f32))> {
    let mut out = Vec::new();
    for (range, _, style) in text_ranges(pieces) {
        let (at, end) = (range.start, range.end);
        if let Some(d) = style.rel_shift
            && d != (0.0, 0.0)
        {
            out.push((at..end, d, style.box_shift.unwrap_or((0.0, 0.0))));
        }
    }
    out
}

/// Сдвиг ПО КУСКАМ: отрезок байт → смещение базовой линии в точках.
///
/// `vertical-align: super`/`sub` поднимает и опускает кусок внутри строки.
/// Доля кегля взята браузерная: треть вверх и пятая часть вниз.
///
/// Центральная доминантная базовая линия повёрнутой строки и `middle` учитываются
/// в `dominant_shift`; `central` — её смещение у струта строки.
pub fn shift_spans(
    pieces: &[Piece],
    base_size: f32,
    line_px: f32,
    opts: &RenderOpts,
    central: Option<f32>,
    block_align: Option<crate::style::computed::Align>,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    for (range, text, style) in text_ranges(pieces) {
        let (at, end) = (range.start, range.end);
        if text.is_empty() {
            continue;
        }
        let size = text_size(style, base_size);
        // К вне-поточной коробке выравнивание в строке не применяется
        // (CSS 2.1 §9.5, §10.8): она из строки вынута.
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            );
        let dy = (!out_of_flow)
            .then_some(style.vertical_shift_px)
            .flatten()
            .or_else(|| {
                // Единица шрифта разрешается ЗДЕСЬ: кегль и гарнитура куска
                // уже известны.
                style.vertical_shift_len.map(|l| {
                    let family = style.font_family.clone().unwrap_or_default();
                    -crate::text::metrics::spacing_px(Some(l), &family, size, opts.cx.vcx())
                })
            })
            .or_else(|| style.vertical_shift.map(|k| k * size))
            // Процент считается от `line-height` САМОГО куска (§10.8.1), а не
            // от кегля: `vertical-align: 50%` при `line-height: 2` — это кегль
            // целиком, а не половина (правка по спеке; в своде такой записи
            // почти нет).
            .or_else(|| {
                style.vertical_shift_pct.map(|k| {
                    let own = match style.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(f)) | Some(Len::Em(f)) => f * size,
                        _ => line_px,
                    };
                    k * own
                })
            })
            .or_else(|| {
                // `text-top`/`text-bottom` равняют край куска по краю
                // ТЕКСТОВОЙ области родителя (CSS 2.1 §10.8.1). Разница
                // берётся из метрик обоих кеглей: подъём и спуск шрифта —
                // доли кегля, поэтому величина пропорциональна их разности.
                // Коробка куска — это глифы ПЛЮС полулидинг с каждой
                // стороны (CSS 2.1 §10.8.1), а у родителя берётся текстовая
                // область без лидинга. Подъём и спуск — доли кегля.
                const ASCENT: f32 = 0.8;
                const DESCENT: f32 = 0.2;
                let parent = style.vertical_align_base.unwrap_or(base_size);
                // Полулидинг БЫВАЕТ отрицательным: `L = line-height − AD`
                // (§10.8.1), и при `line-height` меньше кегля коробка куска
                // выше строки. Зажим нулём убивал ровно этот случай.
                let vertical = style.wm().vertical() || style.wm().turned();
                // CSS 2.1 §10.8.1 aligns the subject's own inline box, whose
                // half-leading comes from its computed line-height. The line's
                // strut is not that box (Blink `inline_box_state.cc`).
                // The edge is physical, so this holds for the central
                // dominant baseline of mixed/upright text too.
                let own_line = if vertical {
                    match style.line_height {
                        Some(Len::Px(v)) => v,
                        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                        None | Some(Len::Auto) => crate::text::metrics::normal_px(
                            crate::text::text_box::normal_fraction(style, opts),
                            size,
                            opts.cx.vcx(),
                        ),
                        _ => line_px,
                    }
                } else {
                    line_px
                };
                let half = (own_line - size) / 2.0;
                style.vertical_align_text.map(|top| {
                    if top {
                        (ASCENT * size + half) - ASCENT * parent
                    } else {
                        DESCENT * parent - (DESCENT * size + half)
                    }
                })
            })
            // Запрет вне-поточной коробке — на ВСЮ цепочку, а не на её голову:
            // `(!out_of_flow).then(..)` гасил только сдвиг длиной в точках, а
            // `sub`/`super`, `em`, `ex`, процент и `text-top`/`text-bottom`
            // проходили дальше по `or_else`. Абсолютный кусок блокифицирован
            // (CSS 2.1 §9.7), `vertical-align` к нему не применяется вовсе.
            // `vertical-align-sub-001`: зелёный кусок уезжал вниз на 0.2em и
            // открывал красный. `super-001` был зелёным случайно: подъём
            // гасила верхняя надбавка строки (`line_padding`).
            .filter(|_| !out_of_flow);
        let dy = if out_of_flow {
            dy
        } else {
            dominant_shift(
                style,
                size,
                base_size,
                central,
                dy,
                block_align,
                opts.cx.vcx(),
            )
        };
        if let Some(v) = dy {
            out.push((at..end, gpui::px(v)));
        }
    }
    out
}

/// Central dominant baseline and `vertical-align: middle` for a text run.
///
/// `central` is the strut's central-baseline offset above its alphabetic
/// baseline when the paragraph is a rotated vertical line set with a central
/// dominant baseline (css-writing-modes-4 §4.2: mixed/upright text). Each run
/// then aligns its own central baseline to the line's (css-inline-3
/// §dominant-baseline): `(a - d) / 2` of the run minus that of the strut.
/// `middle` puts the box midpoint at the parent's baseline raised by half its
/// x-height (CSS 2.1 §10.8.1; Blink inline_box_state.cc kMiddle: in central
/// mode the box's own term is zero and the parent baseline is central).
/// `block_align` is the container's own value (see `edges.rs`).
/// `text-top`/`text-bottom` already name an absolute edge and keep their
/// shift.
fn dominant_shift(
    style: &crate::style::computed::Computed,
    size: f32,
    base_size: f32,
    central: Option<f32>,
    dy: Option<f32>,
    block_align: Option<crate::style::computed::Align>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<f32> {
    let family = style.font_family.clone().unwrap_or_default();
    let own_central = || {
        let (a, d, _) = crate::text::metrics::vmetrics_px(&family, size, vcx);
        (a - d) / 2.0
    };
    // A table cell's own `vertical-align: middle` reaches its text runs
    // through the merged style; it aligns the cell, not an inline box.
    if style.vertical_align == Some(crate::style::computed::Align::Center)
        && style.vertical_align != block_align
    {
        let parent = style.vertical_align_base.unwrap_or(base_size);
        let x_height = crate::text::metrics::ch_ex_px(&family, parent, vcx).1;
        return Some(own_central() - central.unwrap_or(0.0) - x_height / 2.0);
    }
    let Some(strut) = central else {
        return dy;
    };
    if style.vertical_align_text.is_some() {
        return dy;
    }
    let offset = own_central() - strut;
    if offset.abs() < crate::tolerance::TEXT_COORD_EPS {
        return dy;
    }
    Some(dy.unwrap_or(0.0) + offset)
}
