//! Укладка замеренных детей коробки в стопку для `shape_contents`: точки разреза, запреты разрыва, поля.
// owner: A

use crate::layout::fragment::ShapeCx;

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
    mut page_prev: Option<String>,
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
        let mut first = true;
        let mut force_next = false;
        // Сцепка элементов, скованных `avoid*`, — как `avoid_run` в
        // `table_shape`: ОДИН сплошной диапазон от разрешённой границы перед
        // сцепкой до конца её последнего элемента.
        let mut flex_run: Option<f32> = None;
        let mut flex_open = 0.0f32;
        let mut flex_prev_aa = false;
        // Состояние запретов на границах блочных детей (`blk_avoid`).
        let mut blk_prev_aa = false;
        let mut blk_prev_start = top;
        let mut blk_prev_cut: Option<f32> = None;
        // Можно ли начать сцепку от `flex_open`. Нельзя от верха контейнера
        // (css-flexbox-1 §12; Blink `fragmentation_utils.cc:244-253`: без
        // `has_container_separation` — `kBreakAppealLastResort`), от
        // принудительного разрыва (`multi-line-row-flex-fragmentation-023`: рост
        // в `growths` уходил из распорки-коробки в поле) и изнутри строки
        // (`line_inner`).
        let mut flex_open_ok = false;
        // Идёт сцепка (открыта и без диапазона — чтобы её хвост не начал новую
        // с середины).
        let mut flex_chain = false;
        for (ki, (h, kmt, kmb, kcuts, kforced, ksolid, fb, fa, kreach)) in
            kids.into_iter().enumerate()
        {
            // Внепоточный ребёнок гибкого контейнера элементом не является
            // (css-flexbox-1 §4.1): ни зазора, ни границы элементов. Дотяг
            // его низа фрагментации по-прежнему нужен.
            if flex_items && oof_kid.get(ki).copied().unwrap_or(false) {
                let origin = if cx.paged && abs_top.get(ki).copied().unwrap_or(false) {
                    0.0
                } else {
                    y
                };
                oof_reach = oof_reach.max(origin + kreach);
                continue;
            }
            // Сетка: поля рядов не схлопываются ни между собой, ни сквозь
            // верх контейнера (css-grid-1 §6.1), между рядами — `row-gap`.
            // Точка класса A ставится там же, где у блочной стопки, а
            // `cuts` с `nf` за концом зазора продолжает копию с начала
            // следующего ряда — зазор на разрыве пропадает
            // (css-gaps-1 §fragmentation, как в `grid_row_gaps`).
            let lead = if grid_rows_stack {
                if first { kmt } else { prev_mb + row_gap + kmt }
            } else if flex_items {
                // css-flexbox-1 §4.2: поля соседних элементов НЕ схлопываются
                // и сквозь край контейнера не уходят; между ними — `row-gap`.
                if first { kmt } else { prev_mb + flex_gap + kmt }
            } else if first {
                if top == 0.0 {
                    through = kmt;
                    0.0
                } else {
                    kmt
                }
            } else {
                prev_mb.max(kmt)
            };
            if !first && flex_items {
                // Граница элементов — конец ПОЛЯ предыдущего. Поля элементов
                // режутся как содержимое и не усекаются (Blink держит
                // `margin-top` элемента и после разрыва: `single-line-column-
                // flex-fragmentation-033/034`, эталон `-060-print`); усекается
                // только зазор: край внутри зазора уводит разрез к его началу,
                // копия продолжается с его конца — тот же приём, что
                // `grid_row_gaps` ниже (css-gaps-1 §fragmentation).
                // Диапазон зазора начинается РОВНО на границе: с допуском
                // вверх (`b - 0.05`) он перекрывал монолит предыдущего
                // элемента, и `fill_at` шёл по цепочке перекрытий к началу
                // ЭТОГО монолита — разрыв уходил выше целого элемента
                // (`single-line-column-flex-fragmentation-061`: строка Ahem
                // уезжала в следующую колонку вместе с рамкой). Край ровно на
                // `b` по-прежнему режет здесь (`cuts` с тем же `need`).
                let b = y + prev_mb;
                if flex_gap > 0.0 {
                    solid.push((b, b + flex_gap + 0.05));
                }
                cuts.push((b, b + flex_gap));
                if fb || force_next {
                    forced.push(b);
                }
                // css-break-4 §4.3 правило 1: запрет с ЛЮБОЙ стороны границу
                // закрывает, принудительный разрыв открывает обратно.
                let joined =
                    (flex_prev_aa || avoid_kid.get(ki).is_some_and(|a| a.0)) && !(fb || force_next);
                if joined {
                    if !flex_chain {
                        flex_chain = true;
                        flex_run = flex_open_ok.then_some(flex_open);
                    }
                } else {
                    if let Some(s) = flex_run.take() {
                        solid.push((s, y));
                    }
                    flex_chain = false;
                    flex_open = b;
                    flex_open_ok =
                        !(fb || force_next) && !line_inner.get(ki).copied().unwrap_or(false);
                }
            } else if !first {
                cuts.push((y, y + lead));
                // Принудительный разрыв на границе детей.
                if fb || force_next {
                    forced.push(y);
                }
                // Закрытая запретом граница (`blk_avoid`): сплошной диапазон от
                // последней точки внутри предыдущего ребёнка (без неё — от его
                // начала) до начала этого: край колонки в нём уводит разрыв к
                // его началу (`fill_at`, ветка `holds`). От верха коробки
                // диапазон не начинается — там разрыв был бы разрывом ПЕРЕД
                // коробкой, и это решает уровень выше.
                if !grid_rows_stack
                    && (blk_prev_aa || blk_avoid.get(ki).is_some_and(|a| a.0))
                    && !(fb || force_next)
                {
                    let open = blk_prev_cut.unwrap_or(blk_prev_start);
                    if open > top + 0.01 {
                        solid.push((open, y + lead + 0.05));
                    }
                }
            }
            // Смена имени страницы между соседями — принудительный разрыв.
            let renamed = match (&page_prev, page_kid.get(ki).and_then(|p| p.as_ref())) {
                (Some(prev), Some((start, _))) => prev != start,
                _ => false,
            };
            if renamed && !first && !(fb || force_next) {
                forced.push(if flex_items { y + prev_mb } else { y });
            }
            if let Some(Some((_, end))) = page_kid.get(ki) {
                page_prev = Some(end.clone());
            }
            force_next = fa;
            let start = y + lead;
            // Последняя законная точка ВНУТРИ этого ребёнка (для `blk_avoid`).
            blk_prev_start = if first { start } else { y };
            blk_prev_cut = kcuts
                .iter()
                .map(|&(need, _)| need)
                .filter(|&n| n > 0.01 && n < h - 0.01)
                .fold(None::<f32>, |m, n| Some(m.map_or(n, |x| x.max(n))))
                .map(|n| start + n);
            blk_prev_aa = blk_avoid.get(ki).is_some_and(|a| a.1);
            for (need, nf) in kcuts {
                cuts.push((start + need, start + nf));
            }
            for f in kforced {
                forced.push(start + f);
            }
            for (a, b) in ksolid {
                solid.push((start + a, start + b));
            }
            // Дотяг ребёнка — от ЕГО верха; переводим в
            // координаты этой коробки. `y` он не двигает:
            // внепоточный соседей не сдвигает
            // (CSS 2.1 §9.3.1).
            let origin = if cx.paged && abs_top.get(ki).copied().unwrap_or(false) {
                0.0
            } else {
                start
            };
            oof_reach = oof_reach.max(origin + kreach);
            y = start + h;
            prev_mb = kmb;
            first = false;
            flex_prev_aa = avoid_kid.get(ki).is_some_and(|a| a.1);
        }
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
