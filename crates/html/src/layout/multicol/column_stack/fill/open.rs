//! Начало ребёнка в колонке: группы, принудительные разрывы, флоаты, отступ.

use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;

#[allow(clippy::too_many_arguments)]
pub(super) fn open_kid(
    col: &mut usize,
    y: &mut f32,
    prev_mb: &mut f32,
    first: &mut bool,
    not_top: usize,
    placed: &mut bool,
    force_next: &mut bool,
    group: &mut Option<((usize, f32, bool), (usize, f32))>,
    float_hold: &mut Option<(usize, usize, f32)>,
    k: &Kid,
) -> ((usize, f32, bool, f32, bool), f32) {
    if k.par.group != 0
        && k.par.line_start
        && !k.par.group_start
        && let Some(((sc, sy, sp), end)) = group.as_mut().map(|g| (g.0, &mut g.1))
    {
        // Следующая строка — с того же места, что и вся группа; конец
        // предыдущей запомнен (столбец важнее высоты).
        if (*col, *y) > *end {
            *end = (*col, *y);
        }
        *col = sc;
        *y = sy;
        *placed = sp;
        *prev_mb = 0.0;
        *force_next = false;
    }
    // Принудительный разрыв перед коробкой или после предыдущей:
    // новая колонка, если текущая не пуста (css-break-4 §3.1). Разрыв
    // перед ПЕРВЫМ элементом любой строки flex перенесён на сам
    // контейнер (Blink `flex_layout_algorithm.cc:1907-1918`: «Treat all
    // columns as a "row" of columns … propagated to the container»;
    // `split_flex_lines`), и у строки второй раз не действует
    // (`multi-line-column-flex-fragmentation-025`).
    let line_head = k.par.group != 0 && k.par.line_start && !k.par.group_start;
    if (k.force_before && !line_head || *force_next) && *placed {
        *col += 1;
        *y = 0.0;
        *placed = *col < not_top;
        ColumnStack::skip_float(float_hold, col, y, placed);
        *prev_mb = 0.0;
        *first = true;
    }
    if k.par.group != 0 && k.par.group_start {
        // Начало группы — коробка самого контейнера (`split_flex_lines`
        // ставит её первой «строкой»): строки начинаются там, где встал
        // её верх (полей и рамок у контейнера нет, поле предыдущего
        // соседа — сквозь него).
        let gy = if *first { *y } else { *y + *prev_mb };
        *group = Some(((*col, gy, *placed), (*col, gy)));
        *y = gy;
        *prev_mb = 0.0;
    }
    *force_next = k.force_after;
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): схлопывание пары полей по
    // CSS 2.1 §8.3.1 «max положительных + min отрицательных» вместо
    // голого `max`. Срез 3029 пар (css-break/multicol/поля/флоаты
    // CSS2): +0 приобретений, потеря `multi-line-column-flex-
    // fragmentation-032` (0.00 -> 99.00 — страница разъехалась).
    // Правило верное, но в стопке колонок `prev_mb`/`k.mt` уже несут
    // РЕЗУЛЬТАТ схлопывания уровнем выше, и второе применение
    // вычитает отрицательное поле дважды.
    // Элементы строки flex — без схлопывания (css-flexbox-1 §4.2: «The
    // margins of adjacent flex items do not collapse»); первый — от начала
    // группы со своим полем.
    if (k.par.float || k.par.clears)
        && let Some((_, lc, ly)) = float_hold.take()
    {
        *col = lc;
        *y = ly;
        *placed = true;
        *first = false;
        *prev_mb = 0.0;
    }
    let snap = (*col, *y, *placed, *prev_mb, *first);
    let lead = if k.par.group != 0 {
        if k.par.line_start {
            k.mt
        } else {
            *prev_mb + k.mt
        }
    } else if *first {
        k.mt
    } else {
        prev_mb.max(k.mt)
    };
    (snap, lead)
}
