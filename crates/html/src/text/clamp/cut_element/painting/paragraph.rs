//! Last fitting paragraph and line budget for the automatic clamp point.

use super::ClampEntry;

pub(super) fn budget(
    cut: Option<f32>,
    by_count: bool,
    entries: &[ClampEntry],
    empties: &[ClampEntry],
    trim_end: f32,
    fit: &dyn Fn(&ClampEntry) -> Vec<(f32, f32)>,
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
