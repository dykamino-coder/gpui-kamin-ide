//! Геометрия правил промежутков: дорожки и прогоны.
// owner: A

use crate::paint::gap_rules::GapAxisRule;
use gpui::{Bounds, Pixels};

/// Допуск сравнения координат раскладки.
pub(crate) const GAP_EPS: f32 = 0.35;

/// Элемент в осях `a` — поперёк промежутка, `b` — вдоль линейки.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GapItem {
    pub(crate) a0: f32,
    pub(crate) a1: f32,
    pub(crate) b0: f32,
    pub(crate) b1: f32,
}

impl GapItem {
    pub(crate) fn from_bounds(b: &Bounds<Pixels>, gap_on_x: bool) -> Self {
        let x0 = f32::from(b.origin.x);
        let y0 = f32::from(b.origin.y);
        let x1 = x0 + f32::from(b.size.width);
        let y1 = y0 + f32::from(b.size.height);
        if gap_on_x {
            GapItem { a0: x0, a1: x1, b0: y0, b1: y1 }
        } else {
            GapItem { a0: y0, a1: y1, b0: x0, b1: x1 }
        }
    }

    pub(crate) fn flipped(&self) -> Self {
        GapItem { a0: self.b0, a1: self.b1, b0: self.a0, b1: self.a1 }
    }

    /// Заходит ли элемент в участок `[lo, hi]` вдоль линейки.
    pub(crate) fn covers_b(&self, lo: f32, hi: f32) -> bool {
        self.b0 < hi - GAP_EPS && self.b1 > lo + GAP_EPS
    }

    /// Перекрывает ли элемент промежуток `[g0, g1]` (спан через него).
    pub(crate) fn spans_a(&self, g0: f32, g1: f32) -> bool {
        self.a0 <= g0 + GAP_EPS && self.a1 >= g1 - GAP_EPS
    }
}

/// Пересекающий зазор на пути линейки: интервал вдоль `b`; рвёт ли он
/// линейку при `intersection` (видимое пересечение); есть ли в нём поперечная
/// линейка (стык, а не cap) и её ширина.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Crossing {
    pub(crate) lo: f32,
    pub(crate) hi: f32,
    pub(crate) breaks: bool,
    pub(crate) joins: bool,
    pub(crate) cross_w: f32,
}

/// Линейка одного промежутка: интервал промежутка `[g0, g1]`, протяжённость
/// `[r0, r1]`, пересечения, перекрытия спанами, скрытые по visibility участки
/// и характер концов протяжённости — стык (ширина зазора, есть ли линейка,
/// её ширина) или край контейнера (`None`).
#[derive(Clone, Debug)]
pub(crate) struct GapRun {
    pub(crate) g0: f32,
    pub(crate) g1: f32,
    pub(crate) r0: f32,
    pub(crate) r1: f32,
    pub(crate) crossings: Vec<Crossing>,
    pub(crate) blocked: Vec<(f32, f32)>,
    pub(crate) hidden: Vec<(f32, f32)>,
    pub(crate) start_edge: Option<(f32, bool, f32)>,
    pub(crate) end_edge: Option<(f32, bool, f32)>,
    pub(crate) index: usize,
    pub(crate) count: usize,
}

impl GapRun {
    /// Конец отрезка: положение после отступа из пересекающего зазора к его
    /// границе, ширина зазора (0 у края и у «висячего» конца без поперечной
    /// линейки — так считает Blink `GetMaxInsetWidth`), есть ли стык и ширина
    /// поперечной линейки.
    pub(crate) fn edge(&self, pos: f32, is_start: bool) -> (f32, f32, bool, f32) {
        if is_start && (pos - self.r0).abs() <= GAP_EPS {
            return match self.start_edge {
                Some((cw, joins, dw)) => (self.r0, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r0, 0.0, false, 0.0),
            };
        }
        if !is_start && (pos - self.r1).abs() <= GAP_EPS {
            return match self.end_edge {
                Some((cw, joins, dw)) => (self.r1, if joins { cw } else { 0.0 }, joins, dw),
                None => (self.r1, 0.0, false, 0.0),
            };
        }
        if let Some(c) = self
            .crossings
            .iter()
            .find(|c| c.lo - GAP_EPS <= pos && pos <= c.hi + GAP_EPS)
        {
            let at = if is_start { c.hi } else { c.lo };
            return (at, if c.joins { c.hi - c.lo } else { 0.0 }, c.joins, c.cross_w);
        }
        (pos, 0.0, false, 0.0)
    }
}

pub(crate) fn uniq_sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|x, y| (*x - *y).abs() <= GAP_EPS);
    v
}

/// Дорожки по оси `a`: начало — уникальные ближние края элементов, конец —
/// дальний край элемента, начатого в дорожке и не заходящего в следующую;
/// когда такого нет (все — спаны), начало следующей минус зазор.
pub(crate) fn tracks_a(items: &[GapItem], gap: Option<f32>) -> Vec<(f32, f32)> {
    let st = uniq_sorted(items.iter().map(|i| i.a0).collect());
    (0..st.len())
        .map(|k| {
            let next = st.get(k + 1).copied();
            let end = items
                .iter()
                .filter(|i| {
                    (i.a0 - st[k]).abs() <= GAP_EPS && next.is_none_or(|nx| i.a1 <= nx + GAP_EPS)
                })
                .map(|i| i.a1)
                .fold(f32::NEG_INFINITY, f32::max);
            let end = if end.is_finite() {
                end
            } else {
                match (next, gap) {
                    (Some(nx), Some(g)) => nx - g,
                    (Some(nx), None) => nx,
                    (None, _) => st[k],
                }
            };
            (st[k], end)
        })
        .collect()
}

/// Дорожки по ШАБЛОНУ контейнера, привязанные к наблюдённым краям элементов.
///
/// `tracks_a` знает только границы коробок, поэтому дорожка без элементов
/// пропадает: два промежутка вокруг неё сливаются в один широкий, а
/// `§visibility-items: between/around` не может спрятать линейку над пустой
/// областью — хотя спека написана именно про неё («whether a gap decoration
/// segment is painted in portions of gaps adjacent to empty areas»,
/// css-gaps-1 §visibility-items).
///
/// Шаблон даёт РАЗМЕРЫ дорожек, но не их начало на экране; начало ищется
/// перебором: первый наблюдённый край элемента — это начало какой-то из
/// дорожек. Годной считается только та привязка, при которой КАЖДЫЙ элемент
/// стоит краями на линиях сетки. Проверка отсекает поля и выравнивание
/// элемента внутри дорожки, неявные дорожки авто-размещения и
/// нерасшифрованные доли `fr`: там возвращается `None` и работает прежний
/// счёт по элементам. Элемент-спан проверку проходит: его начало — начало
/// первой дорожки пролёта, конец — конец последней.
///
/// Ограничение: сетка, у которой пуста ВСЯ первая дорожка оси, привяжется со
/// сдвигом на дорожку — перебор идёт от нулевого смещения. В своде такой пары
/// нет (у всех 44 разрежённых первая строка и первая колонка заняты).
pub(crate) fn template_tracks(
    sizes: &[f32],
    gap: Option<f32>,
    items: &[GapItem],
) -> Option<Vec<(f32, f32)>> {
    let g = gap?;
    if sizes.len() < 2 {
        return None;
    }
    let mut off = Vec::with_capacity(sizes.len());
    let mut y = 0.0f32;
    for w in sizes {
        off.push(y);
        y += w + g;
    }
    let first = uniq_sorted(items.iter().map(|i| i.a0).collect())
        .first()
        .copied()?;
    for shift in &off {
        let base = first - shift;
        let out: Vec<(f32, f32)> = sizes
            .iter()
            .zip(&off)
            .map(|(w, o)| (base + o, base + o + w))
            .collect();
        let fits = items.iter().all(|i| {
            out.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && out.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        });
        if fits {
            return Some(out);
        }
    }
    None
}

/// Дорожки сетки по x и по y в координатах окна.
pub(crate) type GridTracks = (Vec<(f32, f32)>, Vec<(f32, f32)>);

/// Дорожки без схлопнутых: схлопнутая (`auto-fit` без элементов, css-grid-1
/// §7.2.3.2 «collapsed grid track… the gutters on either side of it…
/// collapse») приходит из раскладки дорожкой нулевого размера, прижатой к
/// соседу без зазора. При ненулевом `gap` такая дорожка — не дорожка и
/// промежутков не даёт (Blink `CollapsedTrackIndexes`).
pub(crate) fn uncollapsed(mut tracks: Vec<(f32, f32)>, gap: f32) -> Vec<(f32, f32)> {
    // При `rtl` и обратных осях раскладка отдаёт дорожки в логическом
    // порядке — здесь нужен физический.
    tracks.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    if gap <= GAP_EPS {
        return tracks;
    }
    let n = tracks.len();
    (0..n)
        .filter(|&k| {
            let (s, e) = tracks[k];
            let empty = e - s <= GAP_EPS;
            let flush_prev = k > 0 && (s - tracks[k - 1].1).abs() <= GAP_EPS;
            let flush_next = k + 1 < n && (tracks[k + 1].0 - e).abs() <= GAP_EPS;
            !(empty && (flush_prev || flush_next))
        })
        .map(|k| tracks[k])
        .collect()
}

/// Промежутки между соседними дорожками; нулевой зазор — тоже промежуток
/// (`flex-gap-decorations-033`).
pub(crate) fn gaps_of(tracks: &[(f32, f32)]) -> Vec<(f32, f32)> {
    tracks
        .windows(2)
        .filter(|w| w[1].0 - w[0].1 >= -GAP_EPS)
        .map(|w| (w[0].1.min(w[1].0), w[1].0))
        .collect()
}

pub(crate) fn merge(mut v: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    v.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<(f32, f32)> = vec![];
    for (lo, hi) in v {
        match out.last_mut() {
            Some(last) if lo <= last.1 + GAP_EPS => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

pub(crate) fn subtract(parts: Vec<(f32, f32)>, (lo, hi): (f32, f32)) -> Vec<(f32, f32)> {
    let mut out = vec![];
    for (s, e) in parts {
        if hi <= s + GAP_EPS || lo >= e - GAP_EPS {
            out.push((s, e));
            continue;
        }
        if lo > s + GAP_EPS {
            out.push((s, lo));
        }
        if hi < e - GAP_EPS {
            out.push((hi, e));
        }
    }
    out
}

/// §visibility-items: заняты ли области по сторонам промежутка `[g0, g1]` в
/// пределах участка `[lo, hi]` вдоль линейки. Спан через промежуток занимает
/// обе стороны.
pub(crate) fn occupied(items: &[GapItem], g0: f32, g1: f32, lo: f32, hi: f32, visibility: u8) -> bool {
    if visibility < 2 {
        return true;
    }
    let near = 2.0 * GAP_EPS;
    let before = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a1 - g0).abs() <= near || i.spans_a(g0, g1)));
    let after = items
        .iter()
        .any(|i| i.covers_b(lo, hi) && ((i.a0 - g1).abs() <= near || i.spans_a(g0, g1)));
    if visibility == 2 {
        before || after
    } else {
        before && after
    }
}

/// Решётка: линейки промежутков оси `a`. Пересекающие зазоры — промежутки
/// оси `b`; пересечение видимо (`breaks`), если хотя бы с одной стороны
/// поперечный зазор не перекрыт спаном (Blink: `kIntersection` идёт дальше
/// только при blocked-before И blocked-after); стык (`joins`) — если там есть
/// видимая поперечная линейка.
pub(crate) fn grid_runs(
    items: &[GapItem],
    gap_a: Option<f32>,
    gap_b: Option<f32>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
    tpl_a: Option<&[f32]>,
    tpl_b: Option<&[f32]>,
    abs: Option<(&[(f32, f32)], &[(f32, f32)])>,
) -> Vec<GapRun> {
    let flipped: Vec<GapItem> = items.iter().map(GapItem::flipped).collect();
    // Дорожки раскладки сильнее всего: это и есть коллекция дорожек сетки
    // (Blink `BuildGridTrackGapData`), с пустыми и схлопнутыми дорожками.
    // Годны, только если каждый элемент стоит краями на линиях дорожек: при
    // `rtl` и вертикальном письме раскладка отдаёт позиции в своей системе
    // отсчёта, и тогда остаётся прежний счёт по элементам и шаблону.
    let on_lines = |tracks: &[(f32, f32)], items: &[GapItem]| {
        items.iter().all(|i| {
            tracks.iter().any(|t| (t.0 - i.a0).abs() <= GAP_EPS)
                && tracks.iter().any(|t| (t.1 - i.a1).abs() <= GAP_EPS)
        })
    };
    if let Some((ta, tb)) = abs
        && !ta.is_empty()
        && !tb.is_empty()
        && on_lines(ta, items)
        && on_lines(tb, &flipped)
    {
        return grid_runs_on(items, &flipped, ta.to_vec(), tb.to_vec(), rule, cross);
    }
    // Дорожки шаблона сильнее выведенных из коробок (css-gaps-1 §gap-grid;
    // Blink `BuildGridTrackGapData` строит геометрию из коллекции дорожек).
    // Привязка не сошлась — остаётся прежний счёт по элементам, картинка не
    // меняется.
    let ta = tpl_a
        .and_then(|t| template_tracks(t, gap_a, items))
        .unwrap_or_else(|| tracks_a(items, gap_a));
    let tb = tpl_b
        .and_then(|t| template_tracks(t, gap_b, &flipped))
        .unwrap_or_else(|| tracks_a(&flipped, gap_b));
    grid_runs_on(items, &flipped, ta, tb, rule, cross)
}

pub(crate) fn grid_runs_on(
    items: &[GapItem],
    flipped: &[GapItem],
    ta: Vec<(f32, f32)>,
    tb: Vec<(f32, f32)>,
    rule: &GapAxisRule,
    cross: Option<&GapAxisRule>,
) -> Vec<GapRun> {
    let ga = gaps_of(&ta);
    let gb = gaps_of(&tb);
    let r0 = tb.first().map_or(0.0, |t| t.0);
    let r1 = tb.last().map_or(0.0, |t| t.1);
    let n = ga.len();
    ga.iter()
        .enumerate()
        .map(|(k, &(g0, g1))| {
            let blocked = merge(
                items
                    .iter()
                    .filter(|i| i.spans_a(g0, g1))
                    .map(|i| (i.b0, i.b1))
                    .collect(),
            );
            let hidden = tb
                .iter()
                .copied()
                .filter(|&(lo, hi)| !occupied(items, g0, g1, lo, hi, rule.visibility))
                .collect();
            let sides = [ta.get(k).copied(), ta.get(k + 1).copied()];
            let crossings = gb
                .iter()
                .enumerate()
                .map(|(j, &(lo, hi))| {
                    let mut breaks = false;
                    let mut joins = false;
                    for side in sides.iter().flatten() {
                        let blocked_here = flipped.iter().any(|i| {
                            i.spans_a(lo, hi) && i.covers_b(side.0, side.1)
                        });
                        if !blocked_here {
                            breaks = true;
                        }
                        if let Some(c) = cross
                            && !blocked_here
                            && c.styles.at(j, gb.len()).unwrap_or(false)
                            && occupied(flipped, lo, hi, side.0, side.1, c.visibility)
                        {
                            joins = true;
                        }
                    }
                    let cross_w = cross.and_then(|c| c.widths.at(j, gb.len())).unwrap_or(0.0);
                    Crossing { lo, hi, breaks, joins, cross_w }
                })
                .collect();
            GapRun {
                g0,
                g1,
                r0,
                r1,
                crossings,
                blocked,
                hidden,
                start_edge: None,
                end_edge: None,
                index: k,
                count: n,
            }
        })
        .collect()
}

/// Строки гибкого контейнера по оси `a`: пересекающиеся протяжённости
/// элементов сливаются в одну строку.
pub(crate) fn line_groups(items: &[GapItem]) -> Vec<(f32, f32)> {
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
        let windows = merge(inner[k].iter().chain(inner[k + 1].iter()).copied().collect());
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
