//! Разрезы между рядами таблицы: avoid/force рядов и групп, охваты rowspan.

use super::{TableBands, row_geometry};
use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;

#[allow(clippy::too_many_arguments)]
pub(super) fn table_row_cuts(
    depth: u8,
    bands: &mut TableBands,
    spacing: f32,
    cell_cx: ShapeCx,
    parts: Vec<(u8, &Element)>,
    rows: Vec<RowRef<'_>>,
    cuts: &mut Vec<(f32, f32)>,
    forced: &mut Vec<f32>,
    solid: &mut Vec<(f32, f32)>,
    y: &mut f32,
    group_open: &mut Option<(usize, f32)>,
    avoid_run: &mut Option<f32>,
    mut prev_open: f32,
    mut prev_aa: bool,
    mut force_next: bool,
    spans: &mut Vec<(usize, usize, f32)>,
    row_box: &mut Vec<(f32, f32)>,
    vcx: ValueCx<'_>,
) -> Option<()> {
    let topology: Vec<_> = rows
        .iter()
        .map(|r| {
            // Consecutive rows outside a row group share the anonymous group,
            // just as table::rows_left_and_cols groups their absent parent.
            let key = if std::ptr::eq(parts[r.group].1, r.row) {
                usize::MAX
            } else {
                r.group
            };
            (key, r.row)
        })
        .collect();
    let geometry = row_geometry(&topology, depth, cell_cx, spacing, vcx)?;
    for (i, r) in rows.iter().enumerate() {
        let start = *y + spacing;
        let h = geometry[i].height;
        for cell in &geometry[i].cells {
            let shape = &cell.shape;
            if !cell.expandable {
                spans.push((i, cell.span, shape.height));
                continue;
            }
            cuts.extend(shape.cuts.iter().map(|&(a, b)| (start + a, start + b)));
            forced.extend(shape.forced.iter().map(|&f| start + f));
            solid.extend(shape.solid.iter().map(|&(a, b)| (start + a, start + b)));
        }
        // css-break-4 §unforced-breaks Rule 1: «may break at a class A break point
        // only if all the 'break-after' and 'break-before' values applicable to
        // this break point allow it, which is when at least one of them forces a
        // break or when none of them forbid it». Значит запрет с ЛЮБОЙ из двух
        // сторон границу закрывает, а принудительный разрыв её открывает обратно
        // (§forced-breaks: «a forced break value effectively overrides any avoid
        // break value that also applies at that break point»). До сих пор мера не
        // читала запреты вовсе: точка класса A ставилась между любой парой рядов,
        // и разрез садился между вторым и третьим рядом `break-avoidance-001…006`
        // вместо первого и второго.
        let joined = i > 0 && (prev_aa || r.ab) && !(r.fb || force_next);
        if i > 0 {
            // Класс A между рядами: разрез по концу предыдущего ряда,
            // продолжение с начала этого (зазор остаётся на новой странице —
            // копия разложена целиком, геометрия сходится). На запрещённой
            // границе точки нет.
            if !joined {
                cuts.push((*y, start));
            }
            if r.fb || force_next {
                forced.push(*y);
            }
        }
        force_next = r.fa;
        // Диапазон `avoid` начинается в точке класса A ПЕРЕД зазором (`y`),
        // а у первого ряда — в нуле: тогда край листа внутри него читается
        // как «разрыв перед таблицей», а не «внутри её верхней рамки» (Blink
        // `FinishFragmentation`: «Avoid breaking inside block-start border …
        // No valid breakpoints there»; `rowgroup-page-break-inside-avoid-1`).
        let open = if i == 0 { 0.0 } else { *y };
        // Одного отсутствия точки класса A мало: `fill_at` (flow.rs) режет ПО
        // КРАЮ КОЛОНКИ (`else { Some(at(edge)) }`), а `cuts` читает только на
        // точное совпадение с краем — они дают усечение поля, а не запрет.
        // Разрез останавливает единственная вещь — сплошной диапазон: ветка
        // `k.solid.iter().find(|&&(a, b)| holds(a, b))` уводит срез к НАЧАЛУ
        // диапазона, накрывшего край. Поэтому сцепка скованных рядов идёт ОДНИМ
        // диапазоном, и открывается он на `open` ПРЕДЫДУЩЕГО ряда: разрыв обязан
        // уйти к разрешённой границе ПЕРЕД ним (`break-avoidance-001`: сцепка
        // (50, 150), срез уходит на 50 — css-tables-3 §breaking-rules «insert some
        // vertical gap between the rows located before and at the overflow point»).
        // Вложенность с `break-inside: avoid` ряда и группы законна: `fill_at` на
        // страницах берёт САМЫЙ ВНЕШНИЙ из накрывших край диапазонов.
        if joined {
            if avoid_run.is_none() {
                *avoid_run = Some(prev_open);
            }
        } else if let Some(s) = avoid_run.take() {
            solid.push((s, *y));
        }
        if r.row.style.cascaded().break_inside_avoid {
            solid.push((open, start + h));
        }
        if let Some((g, gs)) = *group_open
            && g != r.group
        {
            solid.push((gs, *y));
            *group_open = None;
        }
        if r.avoid && group_open.is_none() {
            *group_open = Some((r.group, open));
        }
        prev_open = open;
        prev_aa = r.aa;
        // Полосы первой шапки/подвала (`TableBands`): от верха их первого ряда
        // до низа последнего. Секция-ряд (`is_row` прямо в таблице) — тоже
        // секция своей роли.
        let (kind, sec) = parts[r.group];
        let band = match kind {
            0 => Some((&mut bands.head, &mut bands.head_avoid)),
            2 => Some((&mut bands.foot, &mut bands.foot_avoid)),
            _ => None,
        };
        if let Some((slot, avoid)) = band {
            *slot = Some(match *slot {
                Some((a, _)) => (a, start + h - a),
                None => (start, h),
            });
            *avoid = sec.style.cascaded().break_inside_avoid;
        }
        row_box.push((start, h));
        *y = start + h;
    }
    Some(())
}

// Плоский список рядов: ряд, № группы, avoid группы, разрывы (свои и
// группы — на первом/последнем её ряду), запреты разрыва на КРАЯХ ряда
// (`ab`/`aa` — свои, ячеек и краёв группы).
pub(super) struct RowRef<'a> {
    pub(super) row: &'a Element,
    pub(super) group: usize,
    pub(super) avoid: bool,
    pub(super) fb: bool,
    pub(super) fa: bool,
    pub(super) ab: bool,
    pub(super) aa: bool,
}
