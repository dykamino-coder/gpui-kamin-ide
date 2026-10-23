//! Сегменты правил промежутков строк flex и multicol.

use super::assignment::{RuleSlot, assign};
use super::{Crossing, GAP_RULE_EPS, GapItem, GapRun};
use crate::layout::multicol::gap_rules::GapAxisRule;
use crate::paint::gap_rules::geometry::grid::merge;
use crate::paint::gap_rules::geometry::tracks::tracks_a;

/// Строки гибкого контейнера по оси `a`: пересекающиеся протяжённости
/// элементов сливаются в одну строку.
pub(super) fn line_groups(items: &[GapItem]) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = items.iter().map(|i| (i.a0, i.a1)).collect();
    v.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo < last.1 - GAP_RULE_EPS || (lo - last.0).abs() <= GAP_RULE_EPS => {
                last.1 = last.1.max(hi)
            }
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// Строки/ленты: `a` — ось укладки строк, `b` — ось элементов строки. Главные
/// промежутки — между строками, их пересекающие зазоры — ОБЪЕДИНЕНИЕ зазоров
/// соседних строк (окна перекрытия Blink); поперечные — между соседними
/// элементами строки, протяжённостью в пределах строки, со стыками на её
/// краях. Значения списков: главные — по строкам, поперечные — сквозной счёт.
/// Не считать главный промежуток УЖЕ `gap` не-зазором, а стык без промежутка —
/// не-junction (под ряды многоколонника): это рвёт правила flex/column/row
/// (`flex-gap-decorations-001/019/025/031`, `column-gap-decorations-001/003`,
/// `row-gap-decorations-003/010`); ряды многоколонника обходятся без этого.
// Gap intersections require both axis geometries and their decoration styles.
#[allow(clippy::too_many_arguments)]
pub(crate) fn line_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    main: Option<&GapAxisRule>,
    cross: Option<&GapAxisRule>,
    reverse: (bool, bool),
    extent: Option<(u8, f32, f32)>,
    content_aligned: bool,
    lanes: Option<(Vec<(f32, f32)>, f32)>,
    flex_extent: Option<(f32, f32)>,
) -> (Vec<GapRun>, Vec<GapRun>) {
    // Гибкие строки: строка — объединение поперечных протяжённостей её
    // элементов (Blink: `line_cross_start/end` строки, а не начало каждого
    // элемента), иначе при `align-items: flex-end` элементы разной высоты
    // разбегались по разным «строкам» (`flex-gap-decorations-007`).
    let flex = matches!(extent, Some((1, _, _)));
    // Ленты: полосы — дорожки оси решётки из раскладки; элемент входит в
    // каждую ленту, которую покрывает (элемент во несколько лент — запись в
    // каждой, Blink `GridLanesGapAccumulator::BuildCrossGaps`), а поперечный
    // промежуток стоит сразу перед началом следующей записи ленты: центр —
    // `ForwardStackingStart() - stacking_gap / 2` (`FinalGutterCenter`).
    let lane_gap = lanes.as_ref().map(|(_, g)| *g);
    let mut lines = match &lanes {
        Some((t, _)) => t.clone(),
        None if flex => line_groups(items),
        None => tracks_a(items, gap_a),
    };
    if flex && let Some((start, end)) = flex_extent {
        super::flex_lines::stretch(&mut lines, gap_a.unwrap_or(0.0), start, end);
    }
    let mut r0 = items.iter().map(|i| i.b0).fold(f32::INFINITY, f32::min);
    let mut r1 = items.iter().map(|i| i.b1).fold(f32::NEG_INFINITY, f32::max);
    let inner: Vec<Vec<(f32, f32)>> = lines
        .iter()
        .map(|&(s, e)| {
            let mut row: Vec<&GapItem> = items
                .iter()
                .filter(|i| {
                    if lane_gap.is_some() {
                        i.a0 < e - GAP_RULE_EPS && i.a1 > s + GAP_RULE_EPS
                    } else if flex {
                        (i.a0 - s).abs() <= GAP_RULE_EPS || (i.a0 > s && i.a0 < e - GAP_RULE_EPS)
                    } else {
                        (i.a0 - s).abs() <= GAP_RULE_EPS
                    }
                })
                .collect();
            row.sort_by(|x, y| x.b0.total_cmp(&y.b0));
            if let Some(g) = lane_gap {
                return row
                    .windows(2)
                    .map(|w| super::lane_visibility::cross_window(w[1], g, &lines, items))
                    .collect();
            }
            row.windows(2)
                .filter(|w| w[1].b0 - w[0].b1 >= -GAP_RULE_EPS)
                .map(|w| (w[0].b1.min(w[1].b0), w[1].b0))
                .collect()
        })
        .collect();
    match extent {
        Some((1, c0, c1)) => {
            let last_cross = inner
                .last()
                .and_then(|v| v.last())
                .map(|&(lo, hi)| (lo + hi) / 2.0);
            r0 = r0.min(c0);
            r1 = last_cross.map_or(c1, |x| x.max(c1));
        }
        Some((2, _, _)) if content_aligned => {}
        Some((2, c0, c1)) => {
            r0 = c0;
            r1 = r1.max(c1);
        }
        _ => {}
    }
    let counts: Vec<_> = inner.iter().map(Vec::len).collect();
    let slots = assign(&counts, reverse.0, reverse.1, lane_gap.is_some());
    let main_count = lines.len().saturating_sub(1);
    let main_at = |k| {
        let slot = RuleSlot::new(k, main_count, reverse.0);
        (
            main.and_then(|m| slot.value(&m.styles)).unwrap_or(false),
            main.and_then(|m| slot.value(&m.widths)).unwrap_or(0.0),
        )
    };
    let cross_w = cross.and_then(|c| c.widths.first()).unwrap_or(0.0);
    let mut mains = vec![];
    for k in 0..main_count {
        let (g0, g1) = (lines[k].1, lines[k + 1].0);
        if g1 - g0 < -GAP_RULE_EPS {
            continue;
        }
        let g0 = g0.min(g1);
        let blocked = merge(
            items
                .iter()
                // Элемент flex принадлежит одной строке; касание двух
                // строк при нулевом gap не блокирует правило между ними.
                .filter(|i| !flex && i.spans_a(g0, g1))
                .map(|i| match lane_gap {
                    Some(_) => super::lane_visibility::lane_entry_block(
                        i,
                        &inner[k],
                        &inner[k + 1],
                        (r0, r1),
                        items,
                        [lines[k], lines[k + 1]],
                    ),
                    None => (i.b0, i.b1),
                })
                .collect(),
        );
        let windows = merge(
            inner[k]
                .iter()
                .chain(inner[k + 1].iter())
                .copied()
                .collect(),
        );
        let crossings = windows
            .iter()
            .map(|&(lo, hi)| Crossing {
                lo,
                hi,
                breaks: true,
                joins: cross.is_some() || lane_gap.is_some(),
                cross_w,
            })
            .collect();
        let hidden = match (&lanes, main) {
            (Some(_), Some(m)) => super::lane_visibility::hidden_lane_segments(
                items,
                lines[k],
                lines[k + 1],
                &[inner[k].as_slice(), inner[k + 1].as_slice()].concat(),
                (r0, r1),
                m.visibility,
            ),
            _ => vec![],
        };
        mains.push(GapRun {
            g0,
            g1,
            r0,
            r1,
            crossings,
            blocked,
            hidden,
            start_edge: None,
            end_edge: None,
            index: RuleSlot::new(k, main_count, reverse.0).index,
            count: main_count,
        });
    }
    let mut crosses = vec![];
    for (k, &(s, e)) in lines.iter().enumerate() {
        let before = (k > 0).then(|| {
            let (joins, width) = main_at(k - 1);
            (s - lines[k - 1].1, joins, width)
        });
        let after = (k + 1 < lines.len()).then(|| {
            let (joins, width) = main_at(k);
            (lines[k + 1].0 - e, joins, width)
        });
        for (j, &(lo, hi)) in inner[k].iter().enumerate() {
            crosses.push(GapRun {
                g0: lo,
                g1: hi,
                r0: s,
                r1: e,
                crossings: vec![],
                blocked: vec![],
                hidden: vec![],
                start_edge: before,
                end_edge: after,
                index: slots[k][j].index,
                count: slots[k][j].count,
            });
        }
    }
    // Ленты: поперечный промежуток всегда остаётся в своей ленте и не
    // идёт сквозь главный промежуток, даже если соседние ленты выровнены
    // элементом во несколько лент (WPT css-gaps/grid-lanes
    // `*-gap-decorations-004/006/010/018/019`: поперечные правила режутся
    // по каждой ленте).
    (mains, crosses)
}
