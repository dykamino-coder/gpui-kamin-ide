//! Position for spans; split out to keep the owning module within 250 lines.

use crate::style::values::value::Len;
use crate::text::inline::*;

/// Относительный сдвиг ПО КУСКАМ: отрезок байт → смещение отрисовки.
///
/// Отдельно от `shift_spans`: тот растит строчную коробку, а относительный
/// сдвиг на поток не влияет вовсе (CSS 2.1 §9.4.3).
pub fn rel_spans(pieces: &[Piece]) -> Vec<(std::ops::Range<usize>, (f32, f32))> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        if let Some(d) = style.rel_shift
            && d != (0.0, 0.0)
        {
            out.push((at..end, d));
        }
        at = end;
    }
    out
}

/// Сдвиг ПО КУСКАМ: отрезок байт → смещение базовой линии в точках.
///
/// `vertical-align: super`/`sub` поднимает и опускает кусок внутри строки.
/// Доля кегля взята браузерная: треть вверх и пятая часть вниз.
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-centralbaseline-2026-09.md`,
// 4 хунка): центральная доминантная базовая линия у повёрнутой строки
// (css-writing-modes-4 §4.2: «the central baseline is used as the
// dominant baseline when text-orientation is mixed or upright»,
// величина по css-inline-3 A.2 = (ascent − descent)/2; Blink —
// `computed_style.cc:2154`). Патч включал разделение `mixed` и
// `sideways`, которые у нас хранятся одним булем.
// Срез 1476 пар, база тем же списком: **+0 / −3**. Обещанные
// `text-baseline-vrl-002` и `-vlr-003` НЕ позеленели, а ушли
// `vertical-alignment-003` 0.39 → 0.61, `-009` 0.39 → 0.55,
// `-vlr-025` 0.17 → 1.17. Возвращать только вместе с разбором того,
// почему поправка не даёт нуля там, где арифметика скаута его даёт.
pub fn shift_spans(
    pieces: &[Piece],
    base_size: f32,
    line_px: f32,
) -> Vec<(std::ops::Range<usize>, gpui::Pixels)> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let end = at + text.len();
        let size = match style.font_size {
            Some(Len::Px(v)) => v,
            _ => base_size,
        };
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
                    -crate::text::metrics::spacing_px(Some(l), &family, size)
                })
            })
            .or_else(|| style.vertical_shift.map(|k| k * size))
            // Процент считается от `line-height` САМОГО куска (§10.8.1), а не
            // от кегля: `vertical-align: 50%` при `line-height: 2` — это кегль
            // целиком, а не половина. Замерено: 0 и 0 — правка по спеке, в
            // своде такой записи почти нет.
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
                let half = (line_px - size) / 2.0;
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
        if let Some(v) = dy {
            out.push((at..end, gpui::px(v)));
        }
        at = end;
    }
    out
}
