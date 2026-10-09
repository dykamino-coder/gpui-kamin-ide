//! Полосы сетки при фрагментации.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::{GridSpot, ShapeCx};
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Высота сетки по ЯВНЫМ дорожкам рядов: все дорожки в
/// точках, плюс зазоры между ними. `None` — дорожки
/// неизвестны или не все в точках.
pub(crate) fn grid_rows_px(c: &Computed) -> Option<f32> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(
        c.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let mut total = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => total += v,
            _ => return None,
        }
    }
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    Some(total + gap * (rows.len() as f32 - 1.0))
}

/// Зазоры между ЯВНЫМИ рядами сетки от верха содержимого: `(начало, конец)`.
/// Ряды — в точках либо доли `fr` при заданной в точках высоте коробки
/// (остаток после точечных рядов и зазоров делится по долям, css-grid-1
/// §12.7). Иначе — пусто: дорожек не знаем, точек не даём.
pub(super) fn grid_row_gaps(c: &Computed, inner_h: f32) -> Vec<(f32, f32)> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(
        c.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return Vec::new();
    }
    let Some(rows) = c.grid_rows.as_ref() else {
        return Vec::new();
    };
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    if rows.len() < 2 || gap <= 0.0 {
        return Vec::new();
    }
    let mut fixed = 0.0f32;
    let mut fr = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => fixed += v,
            TrackSize::Single(Track::Fr(k)) => fr += k,
            _ => return Vec::new(),
        }
    }
    let per_fr = if fr > 0.0 {
        if !matches!(c.height, Some(Len::Px(_))) {
            return Vec::new();
        }
        (inner_h - fixed - gap * (rows.len() as f32 - 1.0)).max(0.0) / fr
    } else {
        0.0
    };
    let mut out = Vec::new();
    let mut y = 0.0f32;
    for (i, t) in rows.iter().enumerate() {
        y += match t {
            TrackSize::Single(Track::Px(v)) => *v,
            TrackSize::Single(Track::Fr(k)) => k * per_fr,
            _ => 0.0,
        };
        if i + 1 < rows.len() {
            out.push((y, y + gap));
            y += gap;
        }
    }
    out
}

/// Полосы ЯВНЫХ рядов сетки, когда все дорожки и зазор — в точках:
/// `(начало, конец)` каждого ряда от верха содержимого. Тот же путь, что
/// даёт высоту в `grid_rows_px`, только развёрнутый по рядам: границы рядов
/// — точки разреза класса A (css-grid-2 §Fragmenting Grid Layout: «Class A
/// break opportunities occur between rows or columns»). `None` — дорожек
/// нет, они не все в точках или зазор задан не в точках: границ мы не знаем
/// и точек не даём. Строже, чем `grid_rows_px` (тот считает незнакомый
/// зазор нулём) — неверная граница ряда хуже отсутствующей.
fn grid_px_row_bands(c: &Computed) -> Option<Vec<(f32, f32)>> {
    use crate::style::computed::{Track, TrackSize};
    if !matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let gap = match c.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    let mut out = Vec::with_capacity(rows.len());
    let mut y = 0.0f32;
    for t in rows {
        let TrackSize::Single(Track::Px(v)) = t else {
            return None;
        };
        out.push((y, y + v));
        y += v + gap;
    }
    Some(out)
}

/// Полосы рядов сетки, когда `grid_rows_px` бессилен: колонок больше одной,
/// ряд `auto` или ряд вовсе неявный. `(начало, конец)` каждого ряда от верха
/// содержимого; последний конец — высота сетки.
///
/// css-grid-2 §Fragmenting Grid Layout, «Sample Fragmentation Algorithm»
/// шаг 4: «If the grid height is ''auto'', the height of the grid should be
/// the sum of the final row sizes». Blink считает ровно это —
/// `grid_layout_algorithm.cc:370` `CalculateIntrinsicBlockSize`:
/// `layout_data.Rows().CalculateSetSpanSize() + border_scrollbar_padding
/// .BlockSum()`, без всякого условия «дорожки в точках».
///
/// Размер ряда: явная дорожка `Px` — как есть; `auto` и неявный ряд — по
/// НАИБОЛЬШЕМУ элементу ряда (css-grid-1 §12.5: `auto` как максимум —
/// max-content вклада), с полями: поля элементов сетки не схлопываются
/// (§6.1). Размещение — css-grid-1 §8.5: сперва элементы с ЯВНОЙ линией
/// ряда (шаг 2), затем курсор по рядам (шаг 4).
///
/// Отказ (`None`) — на всём, где догадка была бы неверной: `fr`, проценты,
/// `minmax`, `min-content`, `subgrid`, `repeat(auto-fill …)` в дорожках;
/// `grid-template-areas`; `grid-auto-flow` по колонкам или `dense`;
/// `grid-auto-rows` заданного размера; распределяющий `align-content`;
/// зазор не в точках; явная КОЛОНКА или охват рядов у ребёнка; ребёнок,
/// который сам себя измерить не даёт. Отказ = прежнее поведение, поэтому
/// ни одна пара, что мерится сегодня, этой функции не видит: она стоит
/// ПОСЛЕ `grid_rows_px` в той же ветке.
///
/// Точек разреза функция НЕ даёт нарочно. Класс A между рядами
/// (css-grid-2 §Fragmenting Grid Layout) — возможность, а не предпочтение:
/// Blink переносит ряд в следующий фрагментаинер только при принудительном
/// разрыве (`grid_layout_algorithm.cc:2161-2167`) или при отказе
/// `MovePastBreakpoint` (:2178), а обычный ряд режет по краю. Точка класса A
/// на каждой границе ряда увела бы разрез у зелёных
/// `grid-item-oof-002/003` (ряды `50px 150px`, край колонки на 100 внутри
/// второго ряда) с края на 50 и потеряла бы половину колонки.
pub(super) fn grid_auto_row_bands(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
) -> Option<(Vec<(f32, f32)>, Vec<GridSpot>)> {
    use crate::style::computed::{AutoFlow, Placement, Track, TrackSize};
    let s = &c.style;
    if depth == 0 || !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return None;
    }
    // Именованные области размещаются ниже (область в один ряд); прочее —
    // отказ, как прежде.
    if s.align_content.is_some()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || !matches!(s.grid_auto_rows, None | Some(TrackSize::Single(Track::Auto)))
        || !s.grid_auto_rows_list.is_empty()
    {
        return None;
    }
    let gap = match s.gap {
        None | Some((None, _)) => 0.0,
        Some((Some(Len::Px(v)), _)) => v,
        _ => return None,
    };
    // Явные дорожки рядов: `Some(px)` — размер известен, `None` — ряд `auto`
    // и меряется содержимым. Всё прочее — отказ.
    let mut track: Vec<Option<f32>> = Vec::new();
    if let Some(rows) = s.grid_rows.as_ref() {
        for t in rows {
            match t {
                TrackSize::Single(Track::Px(v)) => track.push(Some(*v)),
                TrackSize::Single(Track::Auto) => track.push(None),
                _ => return None,
            }
        }
    }
    // Колонки: `grid-template-columns` перечислимым списком либо его нет —
    // тогда неявная колонка ровно одна.
    let cols = match (s.grid_cols, s.grid_tracks.as_ref()) {
        (Some(n), _) => n.max(1) as usize,
        (None, Some(t)) => {
            if t.iter().any(|x| matches!(x, TrackSize::AutoRepeat { .. })) {
                return None;
            }
            t.len().max(1)
        }
        (None, None) => 1,
    };
    // Области задают и неявные колонки (css-grid-1 §7.3): `'a b' 'c c'` без
    // `grid-template-columns` — две колонки.
    let cols = cols.max(
        s.grid_areas
            .as_ref()
            .map_or(0, |a| a.iter().map(|r| r.len()).max().unwrap_or(0)),
    );
    // `used[ряд][колонка]` — занятость, `fill[ряд]` — содержимое ряда.
    let mut used: Vec<Vec<bool>> = Vec::new();
    // Элементы с их рядом и мерой — для внутренних точек (`shape_full`) и
    // спуска распорки роста (`pushed_box_at`).
    let mut spots: Vec<GridSpot> = Vec::new();
    let mut fill: Vec<f32> = Vec::new();
    for pass in 0..2u8 {
        let mut cur_row = 0usize;
        let mut cur_col = 0usize;
        for (ix, n) in c.children.iter().enumerate().filter(|(_, n)| !is_blank(n)) {
            let Node::Element(k) = n else {
                return None;
            };
            if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
                continue;
            }
            // `(ряд, начальная колонка, охват колонок)`. Именованная область —
            // её прямоугольник (как `place_named_areas`), только в ОДИН ряд:
            // охват рядов сам меняет размер дорожек (css-grid-1 §12.5).
            // Колонка — css-grid-1 §8.3: `N`, `span M`, `N / span M`, `N / M`;
            // неявные колонки за краем и отрицательные линии — отказ.
            let (line, cline, cspan) = if let Some(name) = &k.style.grid_area_name {
                let areas = s.grid_areas.as_ref()?;
                let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (r, cells) in areas.iter().enumerate() {
                    for (cc, cell) in cells.iter().enumerate() {
                        if cell == name {
                            r0 = r0.min(r);
                            r1 = r1.max(r + 1);
                            c0 = c0.min(cc);
                            c1 = c1.max(cc + 1);
                        }
                    }
                }
                if r0 == usize::MAX || r1 != r0 + 1 {
                    return None;
                }
                (Some(r0), Some(c0), c1 - c0)
            } else {
                let line = match k.style.grid_row {
                    None | Some((Placement::Auto, Placement::Auto)) => None,
                    Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some((a - 1) as usize),
                    _ => return None,
                };
                let (cline, cspan) = match k.style.grid_col {
                    None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                    Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Auto)) if a >= 1 => {
                        (Some((a - 1) as usize), 1usize)
                    }
                    Some((Placement::Line(a), Placement::Span(m))) if a >= 1 => {
                        (Some((a - 1) as usize), m.max(1) as usize)
                    }
                    Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b > a => {
                        (Some((a - 1) as usize), (b - a) as usize)
                    }
                    _ => return None,
                };
                (line, cline, cspan)
            };
            if cspan > cols || cline.is_some_and(|c0| c0 + cspan > cols) {
                return None;
            }
            // Проход 0 — «locked to a given row» (§8.5 шаг 2), проход 1 —
            // курсор (§8.5 шаг 4). Мера ребёнка берётся ровно один раз.
            if (pass == 0) != line.is_some() {
                continue;
            }
            let ks = shape_full(k, depth - 1, cx)?;
            let h = ks.0 + ks.1 + ks.2;
            let busy = |used: &Vec<Vec<bool>>, r: usize, c0: usize| {
                used.get(r).is_some_and(|v| v[c0..c0 + cspan].iter().any(|x| *x))
            };
            let (row, col) = match (line, cline) {
                // Ряд и колонка заданы (область): ячейки как есть — §8.5
                // шаг 1, перекрытие законно.
                (Some(r), Some(c0)) => (r, c0),
                (Some(r), None) => {
                    // «the earliest line index that ensures this item's grid
                    // area will not overlap any occupied grid cells».
                    let mut cc = 0usize;
                    while cc + cspan < cols && busy(&used, r, cc) {
                        cc += 1;
                    }
                    (r, cc)
                }
                // §8.5 шаг 4 «sparse», заданная колонка: курсор-ряд растёт,
                // если колонка левее курсора, и дальше — до свободных ячеек.
                (None, Some(c0)) => {
                    let mut rr = cur_row + usize::from(c0 < cur_col);
                    while busy(&used, rr, c0) {
                        rr += 1;
                    }
                    (rr, c0)
                }
                // Авто-колонка с охватом: первое место от курсора, где
                // свободны все `cspan` ячеек подряд.
                (None, None) => {
                    let mut rr = cur_row;
                    let mut cc = cur_col;
                    loop {
                        if cc + cspan > cols {
                            cc = 0;
                            rr += 1;
                            continue;
                        }
                        if !busy(&used, rr, cc) {
                            break;
                        }
                        cc += 1;
                    }
                    (rr, cc)
                }
            };
            while used.len() <= row {
                used.push(vec![false; cols]);
                fill.push(0.0);
            }
            for x in &mut used[row][col..col + cspan] {
                *x = true;
            }
            fill[row] = fill[row].max(h);
            // Внутренние точки элемента годны сетке, только если его коробка
            // стоит в начале ряда: монолит (`solid_box`) целиком не режется,
            // выравнивание center/end/baseline и `margin-top: auto` сдвигают
            // коробку на величину, которой мера не знает
            // (`grid-item-fragmentation-024`, `-008`).
            let aligned = |a: &Option<crate::style::computed::Align>| {
                matches!(
                    a,
                    Some(crate::style::computed::Align::Center)
                        | Some(crate::style::computed::Align::End)
                        | Some(crate::style::computed::Align::Baseline)
                        | Some(crate::style::computed::Align::AnchorCenter)
                )
            };
            let plain = !solid_box(k)
                && !aligned(&k.style.align_self)
                && !(k.style.align_self.is_none() && aligned(&c.style.align_items))
                && !matches!(k.style.margin.top, Some(Len::Auto));
            spots.push((ix, row, ks.1, ks, plain));
            if line.is_none() {
                cur_row = row;
                cur_col = col + cspan;
                if cur_col >= cols {
                    cur_col = 0;
                    cur_row += 1;
                }
            }
        }
    }
    let rows_n = track.len().max(used.len());
    if rows_n == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(rows_n);
    let mut y = 0.0f32;
    for i in 0..rows_n {
        let h = match track.get(i) {
            Some(Some(v)) => *v,
            _ => fill.get(i).copied().unwrap_or(0.0),
        };
        out.push((y, y + h));
        y += h + gap;
    }
    Some((out, spots))
}

/// Сетка, которую мерит `grid_auto_row_bands`, — та же цепочка, что в
/// `shape_full`: не стопка, высота `auto`, ряды не все в точках.
pub(super) fn grid_items_spotted(c: &Element) -> bool {
    matches!(c.style.display, Some(Display::Grid) | Some(Display::InlineGrid))
        && !grid_stack(c)
        && c.style.height.is_none()
        && grid_rows_px(&c.style).is_none()
}

/// Принудительные разрывы, перенесённые с ЭЛЕМЕНТОВ сетки на границы РЯДОВ
/// (css-grid-2 §Fragmenting Grid Layout: «The 'break-before' and
/// 'break-after' properties on grid items are propagated to their grid
/// row»). Blink `grid_layout_algorithm.cc:1846-1857`:
/// `row_break_between[set_indices.begin] |= item_break_before`,
/// `[set_indices.end] |= item_break_after`, причём оба значения берутся
/// через `InitialBreakBefore`/`FinalBreakAfter` — то есть С ПОТОМКОВ, что у
/// нас делает `edge_break`. Смещения — от верха СОДЕРЖИМОГО коробки и всегда
/// на НАЧАЛЕ ряда, с которого продолжится следующий фрагмент: зазор перед
/// ним съедается разрывом (css-gaps-1 §fragmentation).
///
/// Разрыв перед ПЕРВЫМ рядом и после ПОСЛЕДНЕГО сюда не попадает: спека
/// отдаёт его контейнеру, и его переносит `edge_break`.
///
/// Размещение элементов по рядам считается только там, где оно однозначно
/// (css-grid-1 §8.5, поток `row` без `dense`): именованных областей нет,
/// `grid-row`/`grid-area` у детей нет, число колонок известно. Курсор идёт
/// по колонкам, `grid-column: N / span M` занимает M колонок и при
/// необходимости пинает курсор вперёд; не влезающий в остаток ряда элемент
/// начинает новый ряд. Любая непонятная форма — пустой список, а не догадка.
pub(super) fn grid_row_forced(c: &Element) -> (Vec<f32>, Vec<(f32, f32)>) {
    use crate::style::computed::{AutoFlow, Placement};
    let s = &c.style;
    if matches!(
        s.grid_auto_flow,
        Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
    ) {
        return (Vec::new(), Vec::new());
    }
    let Some(bands) = grid_px_row_bands(s) else {
        return (Vec::new(), Vec::new());
    };
    let cols = s
        .grid_cols
        .map(|n| n.max(1) as usize)
        .or_else(|| s.grid_tracks.as_ref().map(|t| t.len().max(1)))
        .unwrap_or(1);
    // Явный ОДИН ряд элемента: `grid-row: N`, `N / N+1` или именованная
    // область в один ряд. `Some(None)` — ряд по курсору, `None` — форма,
    // которой мы не знаем (отказ целиком, как прежде).
    let own_row = |k: &Element| -> Option<Option<usize>> {
        if let Some(name) = &k.style.grid_area_name {
            let areas = s.grid_areas.as_ref()?;
            let hit: Vec<usize> = areas
                .iter()
                .enumerate()
                .filter(|(_, r)| r.iter().any(|x| x == name))
                .map(|(i, _)| i)
                .collect();
            return match hit.as_slice() {
                [r] => Some(Some(*r)),
                _ => None,
            };
        }
        match k.style.grid_row {
            None | Some((Placement::Auto, Placement::Auto)) => Some(None),
            Some((Placement::Line(a), Placement::Auto)) if a >= 1 => Some(Some((a - 1) as usize)),
            Some((Placement::Line(a), Placement::Line(b))) if a >= 1 && b == a + 1 => {
                Some(Some((a - 1) as usize))
            }
            _ => None,
        }
    };
    let mut items: Vec<(&Element, Option<usize>)> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else {
            return (Vec::new(), Vec::new());
        };
        if matches!(k.style.display, Some(Display::None)) || out_of_flow(&k.style) {
            continue;
        }
        let Some(r) = own_row(k) else {
            return (Vec::new(), Vec::new());
        };
        items.push((k, r));
    }
    // Курсор ниже занятых ячеек не знает: смесь явных рядов с курсорными
    // (css-grid-1 §8.5 шаги 2 и 4 зависят друг от друга) и курсорный элемент
    // при областях — отказ.
    let explicit = items.iter().filter(|(_, r)| r.is_some()).count();
    if (explicit > 0 && explicit < items.len()) || (explicit == 0 && s.grid_areas.is_some()) {
        return (Vec::new(), Vec::new());
    }
    let mut out: Vec<f32> = Vec::new();
    // Ряды с монолитным элементом (css-break-4 §4.1; Blink: элемент, не
    // влезший в остаток, — разрыв ПЕРЕД рядом, `MovePastBreakpoint`,
    // grid_layout_algorithm.cc:2161-2178). Первый ряд — как прежде: разрыв
    // перед ним принадлежит контейнеру.
    let mut mono: Vec<(f32, f32)> = Vec::new();
    let mut row = 0usize;
    let mut col = 0usize;
    for (k, fixed) in items {
        let r = match fixed {
            Some(r) => r,
            None => {
                let (line, span) = match k.style.grid_col {
                    None | Some((Placement::Auto, Placement::Auto)) => (None, 1usize),
                    Some((Placement::Span(m), Placement::Auto)) => (None, m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Span(m))) => (Some(a), m.max(1) as usize),
                    Some((Placement::Line(a), Placement::Auto)) => (Some(a), 1usize),
                    Some((Placement::Line(a), Placement::Line(b))) => {
                        (Some(a.min(b)), (b - a).unsigned_abs().max(1) as usize)
                    }
                    _ => return (Vec::new(), Vec::new()),
                };
                if span > cols {
                    return (Vec::new(), Vec::new());
                }
                if let Some(a) = line {
                    if a < 1 {
                        return (Vec::new(), Vec::new());
                    }
                    let want = (a - 1) as usize;
                    if want + span > cols {
                        return (Vec::new(), Vec::new());
                    }
                    if want < col {
                        row += 1;
                    }
                    col = want;
                } else if col + span > cols {
                    row += 1;
                    col = 0;
                }
                let here = row;
                col += span;
                if col >= cols {
                    row += 1;
                    col = 0;
                }
                here
            }
        };
        if r >= bands.len() {
            if fixed.is_some() {
                continue;
            }
            break;
        }
        if r > 0 && edge_break(k, false) {
            out.push(bands[r].0);
        }
        if r + 1 < bands.len() && edge_break(k, true) {
            out.push(bands[r + 1].0);
        }
        if r > 0 && (k.style.break_inside_avoid || size_monolith(k)) {
            mono.push(bands[r]);
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    (out, mono)
}

/// Сетка, которую фрагментация вправе спускать СТОПКОЙ: одна колонка, ряды
/// по содержимому, дети без явного размещения. Тогда ряд — ровно один
/// ребёнок, высота ряда равна мере ребёнка (css-grid-1 §11.8: дорожка
/// `auto` — по max-content), а порядок рядов равен порядку детей (§8.5
/// auto-placement при `grid-auto-flow: row`). Поля рядов НЕ схлопываются
/// (§6.1: «margins of grid items do not collapse»), между рядами стоит
/// `row-gap`.
///
/// Отказ (прежний путь — `grid_rows_px`, чаще всего `None`): явные дорожки
/// рядов не все `auto` (px/`fr`/`minmax` — размер ряда не равен мере
/// ребёнка), колонок больше одной (дети параллельны, а не стопкой),
/// именованные области, поток по колонкам или `dense`, неявные ряды
/// заданного размера, распределяющий `align-content` (двигает ряды внутри
/// заданной высоты), зазор не в точках, явное размещение у любого ребёнка
/// (`grid-row`/`grid-column`/`grid-area`). `display: grid-lanes` не
/// проходит никогда — у полос своя укладка.
pub(crate) fn grid_stack(c: &Element) -> bool {
    use crate::style::computed::{AutoFlow, Track, TrackSize};
    let s = &c.style;
    if !matches!(s.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return false;
    }
    if s.grid_cols.unwrap_or(1) > 1
        || s.grid_tracks.as_ref().is_some_and(|t| t.len() > 1)
        || s.grid_areas.is_some()
    {
        return false;
    }
    if let Some(rows) = s.grid_rows.as_ref() {
        // Ряд, чей размер при `height: auto` равен вкладу ЕДИНСТВЕННОГО
        // элемента ряда: `auto`/`min-content`/`max-content` (css-grid-1
        // §12.4-12.6); `minmax(<0 | по содержимому>, <auto | max-content |
        // fr>)` — база не больше вклада, предел = вклад; `fr` при
        // неопределённом свободном месте — §12.7.1 «max-content contribution»
        // (ровно вклад, пока гибкая дорожка ОДНА); `minmax(<по содержимому>,
        // <px>)` — база = min-content = вклад (расходится лишь для элемента
        // ниже предела; копия кладётся `Definite(h)` и берёт ту же высоту).
        // css-grid-2 §12.1 шаг 3 растит именно такие ряды. С заданной
        // высотой `fr`/`minmax` делят ЕЁ, а не вклад: `grid-item-
        // fragmentation-014/016` (`height:200px`) держатся на прежнем пути.
        let content = |t: &Track| matches!(t, Track::Auto | Track::MinContent | Track::MaxContent);
        let lo_ok = |t: &Track| content(t) || matches!(t, Track::Px(v) if *v <= 0.0);
        let by_item = |t: &TrackSize| match t {
            TrackSize::Single(t) => content(t) || matches!(t, Track::Fr(_)),
            TrackSize::MinMax(lo, hi) => {
                lo_ok(lo)
                    && (content(hi)
                        || matches!(hi, Track::Fr(_))
                        || (content(lo) && matches!(hi, Track::Px(_))))
            }
            TrackSize::AutoRepeat { .. } => false,
        };
        let all_auto = rows
            .iter()
            .all(|t| matches!(t, TrackSize::Single(Track::Auto)));
        let flexible = rows
            .iter()
            .filter(|t| {
                matches!(t, TrackSize::Single(Track::Fr(_)) | TrackSize::MinMax(_, Track::Fr(_)))
            })
            .count();
        if !all_auto
            && (!matches!(s.height, None | Some(Len::Auto))
                || s.max_height.is_some()
                || flexible > 1
                || !rows.iter().all(by_item))
        {
            return false;
        }
    }
    if !matches!(
        s.grid_auto_rows,
        None | Some(TrackSize::Single(Track::Auto))
    ) || !s.grid_auto_rows_list.is_empty()
        || matches!(
            s.grid_auto_flow,
            Some(AutoFlow::Col) | Some(AutoFlow::ColDense) | Some(AutoFlow::RowDense)
        )
        || s.align_content.is_some()
    {
        return false;
    }
    if !matches!(s.gap, None | Some((None, _)) | Some((Some(Len::Px(_)), _))) {
        return false;
    }
    !c.children.iter().any(|n| match n {
        Node::Element(k) => {
            k.style.grid_row.is_some()
                || k.style.grid_col.is_some()
                || k.style.grid_area_name.is_some()
        }
        _ => false,
    })
}

/// «Сдвиг ряда» сетки с рядами в точках (Blink `row_offset_adjustments`,
/// grid_layout_algorithm.cc:2304-2337: ряд, начатый в следующем
/// фрагментаинере, сдвигается на остаток предыдущего): разрез ровно на начале
/// ряда `i ≥ 1` (монолитный ряд или перенесённый `break-*`) растит дорожку
/// `i−1` на `grow`. Высота `auto` вырастает на то же (`grid_rows_px`),
/// заданная — нет, и ряд `i` всё равно встаёт на край колонки
/// (`grid-item-oof-004`: абсолют `align-self: end` в ряду 2 — в колонке 2).
/// Только без `row-gap`: при зазоре эталоны css-gaps держат ряд прежней
/// высоты (`grid-gap-decorations-fragmentation-011`). Прямой ребёнок стопки;
/// вложенная сетка — как прежде, без роста.
pub(super) fn grow_grid_track(c: &mut Element, at: f32, grow: f32) -> bool {
    use crate::style::computed::{Track, TrackSize};
    if grid_stack(c) || !matches!(c.style.display, Some(Display::Grid) | Some(Display::InlineGrid)) {
        return false;
    }
    let gap0 = match c.style.gap {
        None | Some((None, _)) => true,
        Some((Some(Len::Px(v)), _)) => v.abs() < 0.01,
        _ => false,
    };
    if !gap0 {
        return false;
    }
    let Some(bands) = grid_px_row_bands(&c.style) else {
        return false;
    };
    let px_of = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let top = px_of(&c.style.padding.top) + px_of(&c.style.borders().top);
    let Some(i) = bands.iter().position(|b| (top + b.0 - at).abs() < 0.01) else {
        return false;
    };
    if i == 0 {
        return false;
    }
    let Some(TrackSize::Single(Track::Px(v))) =
        c.style.grid_rows.as_mut().and_then(|r| r.get_mut(i - 1))
    else {
        return false;
    };
    *v += grow;
    true
}
