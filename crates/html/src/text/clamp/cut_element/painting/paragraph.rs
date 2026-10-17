//! Select the last paragraph and its fitting line budget for automatic clamping.

use super::ClampEntry;

// ★ ЗАМЕРЕНО (10.09, `scout-clampmarker-2026-09c.md`, 21 хунк):
// срез 416 пар (семья `line-clamp` + схлопывание полей) 259 ->
// 260. Взяты `line-clamp-auto-003` 1.82 -> 0.00 и `-047`
// 1.62 -> 0.12; четвёрка `-018`…`-021` стояла на 0.39-0.40 и
// встала РОВНО на 0.00 — знак наконец в конце текста строки.
// Потеря одна: `-022` 0.39 -> 2.61 — тот же тест, что `-021`,
// но `max-height: 5.5lh` вместо `5lh` при том же эталоне;
// полстроки сверх бюджета у нас пускают лишнюю строку. Долг
// отдельным подкорнем CLAMP-HALF-LINE.
// Знак обрыва в АВТО-режиме (`limit == None`). Рисует его НЕ этот
// слой: сюда возвращается только БЮДЖЕТ строк — номер абзаца в
// контейнере и сколько его строк остаётся выше среза, — а «…»
// ставит уже проверенный `lines::clamp_lines`/`paint_line`: в
// конце ТЕКСТА строки, с выключкой и направлением письма
// (css-overflow-4: знак «is placed at the end of the line box
// reducing the space available to the other contents of the
// line», а для bidi — анонимный строчный с уровнем bidi-абзаца).
// Так же развязан и Blink: блочный слой отдаёт признак
// `IsAtClampPoint`, а ширину знака получает разрыватель строк
// (`inline_layout_algorithm.cc:1247` `SetupLineClampEllipsis` →
// `SetLineClampEllipsisWidth`). Прежний набросок рисовал знак у
// ПРАВОГО края коробки — мимо конца текста, мимо выключки и
// мимо rtl.
// Числовой предел, который `max-height` перехватил раньше N-й строки
// (`line-clamp: 4 auto` при `max-height: 3lh`), — та же точка
// обрыва, что в авто-режиме: §5.3 берёт ПЕРВУЮ из двух точек, и знак
// встаёт на последнюю строку перед ней (`line-clamp-041`).
pub(super) fn automatic_para(
    cut: Option<f32>,
    by_count: bool,
    entries: &[ClampEntry],
    empties: &[ClampEntry],
    fit: impl Fn(&ClampEntry) -> Vec<(f32, f32)>,
    trim_end: f32,
) -> Option<(u32, usize)> {
    cut.filter(|_| !by_count).and_then(|c| {
        // Есть ли что резать. Как только бюджет применён, абзац УЖЕ
        // укорочен и сам за срез не выходит — признак защёлкивается
        // применённым бюджетом, иначе кадры зациклились бы:
        // обрезали → влезло → сняли → снова не влезло.
        let overflow = entries.iter().any(|e| e.clamped.is_some())
            || entries
                .iter()
                .any(|e| f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height) > c + 0.5);
        if !overflow {
            return None;
        }
        // Знак садится на ПОСЛЕДНЮЮ строку перед точкой среза — в том
        // числе когда точка стоит МЕЖДУ блоками и сам абзац видим
        // целиком. Абзацы в своём контексте форматирования
        // пропускаются: точкой среза их строки быть не могут.
        // Последняя строка перед точкой — во ВЛОЖЕННОМ контексте
        // форматирования (несчитаемая): знак не ставится вовсе, а не
        // уходит на предыдущий считаемый абзац (css-overflow-4 §5.3:
        // многоточие — на последней строке ПЕРЕД точкой среза в этом
        // BFC; `line-clamp-auto-034/039`: «Line 4» без знака).
        entries
            .iter()
            .filter(|e| e.line > 0.0 && (e.skip_count || e.seq.is_some()))
            .filter_map(|e| {
                let seq = if e.skip_count { None } else { e.seq };
                let h = f32::from(e.bounds.size.height);
                if h <= 0.0 {
                    return None;
                }
                let rows = fit(e);
                // Точка `c` уже стоит на СРЕЗАННОМ низу последней
                // строки — полный низ этой строки ниже на `trim_end`.
                let k = rows
                    .iter()
                    .filter(|(_, b)| *b <= c + trim_end + 0.5)
                    .count();
                (k >= 1).then(|| (rows[k - 1].1, seq, k))
            })
            // The clamp point follows the LAST fitting line in box-tree
            // order: on equal bottoms the later paragraph wins
            // (`line-clamp-auto-043`: a line beside an earlier block's
            // overflow).
            .max_by(|a, b| {
                if (a.0 - b.0).abs() <= 0.5 {
                    a.1.cmp(&b.1)
                } else {
                    a.0.total_cmp(&b.0)
                }
            })
            // Пустая блочная коробка между последней строкой и точкой
            // среза: точка — после неё (последняя возможная), и строка
            // точке уже не предшествует — знака нет (§5.3).
            // Not when that line is where WE cut the paragraph: the
            // empty box then sits right after the cut only because the
            // clamped paragraph got shorter, and the point stays in the
            // paragraph (`line-clamp-with-abspos-021`,
            // `-with-fixed-pos-016`).
            .filter(|(bottom, seq, _)| {
                seq.is_some_and(|s| {
                    entries
                        .iter()
                        .any(|e| e.seq == Some(s) && e.clamped.is_some())
                }) || !empties.iter().any(|e| {
                    let y0 = f32::from(e.bounds.origin.y);
                    e.empty
                        && f32::from(e.bounds.size.height) <= 0.5
                        && y0 >= *bottom - 0.5
                        && y0 <= c + 0.5
                })
            })
            .and_then(|(_, seq, k)| seq.map(|s| (s, k)))
    })
}
