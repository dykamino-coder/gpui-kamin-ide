//! Укладка замеренных детей коробки в стопку для `shape_contents`: точки разреза, запреты разрыва, поля.
// owner: A

use crate::layout::fragment::ShapeCx;
mod cuts;
use cuts::stack_kid_cuts;

#[allow(clippy::too_many_arguments)]
pub(super) fn stack_kids(
    kids: Vec<(
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        f32,
    )>,
    row_nowrap: bool,
    top: f32,
    cuts: &mut Vec<(f32, f32)>,
    forced: &mut Vec<f32>,
    solid: &mut Vec<(f32, f32)>,
    flex_items: bool,
    oof_kid: Vec<bool>,
    cx: ShapeCx,
    abs_top: Vec<bool>,
    grid_rows_stack: bool,
    row_gap: f32,
    flex_gap: f32,
    avoid_kid: Vec<(bool, bool)>,
    line_inner: Vec<bool>,
    blk_avoid: Vec<(bool, bool)>,
    page_kid: Vec<Option<(std::string::String, std::string::String)>>,
    mut oof_reach: f32,
    page_prev: Option<String>,
) -> (Option<(f32, f32, f32)>, f32) {
    let stacked: Option<(f32, f32, f32)>;
    let inner_h: Vec<f32> = kids.iter().map(|k| k.0).collect();
    // Ряд flex БЕЗ переноса: дети стоят бок о бок, и
    // каждый фрагментируется СВОИМИ точками (Blink
    // `flex_layout_algorithm.cc`: элементу строки
    // выдаётся своя доля фрагментаинера). Значит точки
    // ряда — объединение точек детей, а запрет разрыва
    // — объединение их монолитных диапазонов: рвать
    // нельзя там, где не даёт хоть один. Прежде ряд
    // объявлялся монолитом целиком, и разреза не было
    // никогда (`single-line-row-flex-fragmentation-*`).
    if row_nowrap {
        let tallest = inner_h.iter().copied().fold(0.0f32, f32::max);
        for k in &kids {
            let start = top;
            for (need, nf) in &k.3 {
                cuts.push((start + need, start + nf));
            }
            for f in &k.4 {
                forced.push(start + f);
            }
            for (a, b) in &k.5 {
                solid.push((start + a, start + b));
            }
        }
        stacked = Some((top + tallest, 0.0, 0.0));
    } else {
        let mut y = top;
        let mut prev_mb = 0.0f32;
        let mut through = 0.0f32;
        let first = true;
        let force_next = false;
        // Сцепка элементов, скованных `avoid*`, — как `avoid_run` в
        // `table_shape`: ОДИН сплошной диапазон от разрешённой границы перед
        // сцепкой до конца её последнего элемента.
        let mut flex_run: Option<f32> = None;
        let flex_open = 0.0f32;
        let flex_prev_aa = false;
        // Состояние запретов на границах блочных детей (`blk_avoid`).
        let blk_prev_aa = false;
        let blk_prev_start = top;
        let blk_prev_cut: Option<f32> = None;
        // Можно ли начать сцепку от `flex_open`. Нельзя от верха контейнера
        // (css-flexbox-1 §12; Blink `fragmentation_utils.cc:244-253`: без
        // `has_container_separation` — `kBreakAppealLastResort`), от
        // принудительного разрыва (`multi-line-row-flex-fragmentation-023`: рост
        // в `growths` уходил из распорки-коробки в поле) и изнутри строки
        // (`line_inner`).
        let flex_open_ok = false;
        // Идёт сцепка (открыта и без диапазона — чтобы её хвост не начал новую
        // с середины).
        let flex_chain = false;
        stack_kid_cuts(
            kids,
            top,
            cuts,
            forced,
            solid,
            flex_items,
            oof_kid,
            cx,
            abs_top,
            grid_rows_stack,
            row_gap,
            flex_gap,
            avoid_kid,
            line_inner,
            blk_avoid,
            page_kid,
            &mut oof_reach,
            page_prev,
            &mut y,
            &mut prev_mb,
            &mut through,
            first,
            force_next,
            &mut flex_run,
            flex_open,
            flex_prev_aa,
            blk_prev_aa,
            blk_prev_start,
            blk_prev_cut,
            flex_open_ok,
            flex_chain,
        );
        // Сцепка, дожившая до последнего элемента, закрывается его низом.
        if let Some(s) = flex_run {
            solid.push((s, y));
        }
        // Нижнее поле последнего ряда наружу не схлопывается и входит в
        // высоту сетки (css-grid-1 §6.1).
        // Нижнее поле последнего ЭЛЕМЕНТА гибкого контейнера тоже входит в
        // его высоту (внешний размер элемента, css-flexbox-1 §9.4).
        if grid_rows_stack || flex_items {
            y += prev_mb;
            prev_mb = 0.0;
        }
        stacked = Some((y, through, prev_mb));
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): ряд flex С ПЕРЕНОСОМ
        // как строки — жадная сборка по ширинам детей в точках
        // (css-flexbox-1 §9.3), разрез между строками, высота —
        // сумма строк: срез фрагментации 469 -> 469 (0/0) —
        // ширины элементов в тестах не в точках (`flex: 1`,
        // проценты), ветка не срабатывает. Нужна ширина из
        // раскладки, а не из стиля (корень R4 scout-flexfrag).
    }
    (stacked, oof_reach)
}
