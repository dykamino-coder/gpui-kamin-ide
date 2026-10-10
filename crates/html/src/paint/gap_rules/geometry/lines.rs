//! Сегменты правил промежутков строк flex и multicol.

use super::{Crossing, GAP_EPS, GapItem, GapRun};
use crate::paint::gap_rules::GapAxisRule;
use crate::paint::gap_rules::geometry::grid::merge;
use crate::paint::gap_rules::geometry::tracks::tracks_a;

/// Строки гибкого контейнера по оси `a`: пересекающиеся протяжённости
/// элементов сливаются в одну строку.
pub(super) fn line_groups(items: &[GapItem]) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = items.iter().map(|i| (i.a0, i.a1)).collect();
    v.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo < last.1 - GAP_EPS || (lo - last.0).abs() <= GAP_EPS => {
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
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v98, `scout-columnwrap-2026-09b.md` I1):
/// считать главный промежуток УЖЕ `gap` не зазором, а стык без промежутка —
/// не junction (под ряды многоколонника). css-gaps 349: +12/−19 —
/// `flex-gap-decorations-001/019` 99.00, `-025/031/032/035/065…067`,
/// `column-gap-decorations-001/003/014/016/019`, `row-gap-decorations-003/010`.
/// Ряды многоколонника обходятся без него (v99: +11/−0).
pub(crate) fn line_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    main: Option<&GapAxisRule>,
    cross: Option<&GapAxisRule>,
    rev_cross: bool,
    extent: Option<(u8, f32, f32)>,
    content_aligned: bool,
    lanes: Option<(Vec<(f32, f32)>, f32)>,
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
    let lines = match &lanes {
        Some((t, _)) => t.clone(),
        None if flex => line_groups(items),
        None => tracks_a(items, gap_a),
    };
    let mut r0 = items.iter().map(|i| i.b0).fold(f32::INFINITY, f32::min);
    let mut r1 = items.iter().map(|i| i.b1).fold(f32::NEG_INFINITY, f32::max);
    let inner: Vec<Vec<(f32, f32)>> = lines
        .iter()
        .map(|&(s, e)| {
            let mut row: Vec<&GapItem> = items
                .iter()
                .filter(|i| {
                    if lane_gap.is_some() {
                        i.a0 < e - GAP_EPS && i.a1 > s + GAP_EPS
                    } else if flex {
                        i.a0 >= s - GAP_EPS && i.a0 <= e + GAP_EPS
                    } else {
                        (i.a0 - s).abs() <= GAP_EPS
                    }
                })
                .collect();
            row.sort_by(|x, y| x.b0.partial_cmp(&y.b0).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(g) = lane_gap {
                return row.windows(2).map(|w| (w[1].b0 - g, w[1].b0)).collect();
            }
            row.windows(2)
                .filter(|w| w[1].b0 - w[0].b1 >= -GAP_EPS)
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
    let cross_total: usize = inner.iter().map(Vec::len).sum();
    let main_count = lines.len().saturating_sub(1);
    let main_w = main.and_then(|m| m.widths.first()).unwrap_or(0.0);
    let cross_w = cross.and_then(|c| c.widths.first()).unwrap_or(0.0);
    let mut mains = vec![];
    for k in 0..main_count {
        let (g0, g1) = (lines[k].1, lines[k + 1].0);
        if g1 - g0 < -GAP_EPS {
            continue;
        }
        let g0 = g0.min(g1);
        let blocked = merge(
            items
                .iter()
                .filter(|i| i.spans_a(g0, g1))
                .map(|i| (i.b0, i.b1))
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
                joins: cross.is_some(),
                cross_w,
            })
            .collect();
        mains.push(GapRun {
            g0,
            g1,
            r0,
            r1,
            crossings,
            blocked,
            hidden: vec![],
            start_edge: None,
            end_edge: None,
            index: k,
            count: main_count,
        });
    }
    let mut crosses = vec![];
    let mut ix = 0usize;
    for (k, &(s, e)) in lines.iter().enumerate() {
        let before = (k > 0).then(|| (s - lines[k - 1].1, main.is_some(), main_w));
        let after = (k + 1 < lines.len()).then(|| (lines[k + 1].0 - e, main.is_some(), main_w));
        // Счёт сквозной по строкам (§assigning: «does not restart at the
        // beginning of each flex line»), внутри строки — от её логического
        // начала: при `rev_cross` крайний правый (нижний) промежуток первый.
        let n = inner[k].len();
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
                index: if rev_cross { ix + n - 1 - j } else { ix + j },
                count: cross_total.max(1),
            });
        }
        ix += n;
    }
    // Ленты: поперечный промежуток, к которому примыкает элемент во обе
    // соседние ленты, идёт сквозь главный промежуток между ними — тот у
    // этого пересечения перекрыт (Blink `MarkBlockedMainGapSegments`: отрезок
    // главного промежутка, по обе стороны которого одна и та же запись,
    // заблокирован). Такие поперечные прогоны соседних лент сливаются в один.
    if lane_gap.is_some() && lines.len() > 1 {
        let mut k = 0;
        while k < crosses.len() {
            let a = &crosses[k];
            let lane = lines.iter().position(|&(_, e)| (e - a.r1).abs() <= GAP_EPS);
            let joined = lane.filter(|&l| l + 1 < lines.len()).and_then(|l| {
                let (g0, g1) = (lines[l].1, lines[l + 1].0);
                let spanned = items.iter().any(|i| {
                    i.spans_a(g0, g1)
                        && ((i.b1 - a.g0).abs() <= GAP_EPS || (i.b0 - a.g1).abs() <= GAP_EPS)
                });
                if !spanned {
                    return None;
                }
                crosses.iter().position(|b| {
                    (b.r0 - lines[l + 1].0).abs() <= GAP_EPS
                        && (b.g0 - a.g0).abs() <= GAP_EPS
                        && (b.g1 - a.g1).abs() <= GAP_EPS
                })
            });
            if let Some(j) = joined {
                let b = crosses.remove(j);
                let a = &mut crosses[if j < k { k - 1 } else { k }];
                a.r1 = b.r1;
                a.end_edge = b.end_edge;
                // Слитый прогон может слиться и со следующей лентой.
                if j < k {
                    k -= 1;
                }
                continue;
            }
            k += 1;
        }
    }
    (mains, crosses)
}
