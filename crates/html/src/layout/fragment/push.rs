//! Перенос коробок на следующий фрагмент.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::{clone_dec, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{grid_auto_row_bands, grid_items_spotted, grid_stack, grow_grid_track};
use crate::layout::fragment::line_shape::basis_sized;
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::table_bands::{repeat_leads, table_box};
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::multicol::spanner::parallel_items_inside;
use crate::layout::table::anon::fixup_table_children;
use crate::layout::table::is_cell;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Самая внешняя коробка, начинающаяся ровно в `a` от верха `c`, — перед
/// ней встаёт распорка роста (`grow_pushed`). Смещения детей — той же
/// арифметикой, что в `shape_full`: `lead` = схлопнутое поле, у первого
/// без отбивки поле уходит сквозь верх, внепоточный — нулевая запись, ряд
/// flex без переноса — все дети с верха. Сетка, таблица, ряды — без
/// спуска, как там; таблица по тегу — рядами `table_shape`
/// (`pushed_cell_at`). Текст или строчный среди детей — у `shape_full`
/// отказ от спуска, и здесь тоже.
thread_local! {
    /// Щуп `pushed_box_at` ищет коробку под ПРИНУДИТЕЛЬНЫЙ разрыв (`grow_pushed`).
    pub(crate) static PUSH_FORCED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn pushed_box_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
    if depth == 0 {
        return None;
    }
    if table_box(c) {
        return pushed_cell_at(c, a, depth);
    }
    // Сетка-стопка спускается ровно как в `shape_full` — иначе распорка
    // роста (`705fd58`) до ряда сетки не добирается, отдаёт `None`, и фон
    // коробки не дотягивается до низа колонки.
    let grid_rows_stack = grid_stack(c);
    let row_gap = if grid_rows_stack {
        match c.style.gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    if (matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    ) && !grid_rows_stack
        && !grid_items_spotted(c))
        || matches!(c.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
    {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let b = c.style.borders();
    let top = px(&c.style.padding.top) + px(&b.top);
    // Сетка `grid_auto_row_bands`: коробка ищется в элементах ряда,
    // накрывающего `a`, — теми же смещениями, что в мере (`shape_full`).
    // Точка ровно на верху элемента уходит в его первого ребёнка.
    if grid_items_spotted(c) {
        let (bands, spots) = grid_auto_row_bands(c, depth, ShapeCx::COLUMNS)?;
        for (ix, row, kmt, s, plain) in spots {
            let (true, Some(&(r0, _)), Some(Node::Element(k))) =
                (plain, bands.get(row), c.children.get(ix))
            else {
                continue;
            };
            let start = top + r0 + kmt;
            if a > start - 0.01 && a < start + s.0 - 0.01 {
                if let Some(id) = pushed_box_at(k, a - start, depth - 1) {
                    return Some(id);
                }
            }
        }
        return None;
    }
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None
                | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // МНОГОСТРОЧНЫЙ flex: элемент — не коробка стопки, а член СТРОКИ. Распорка
    // перед самим элементом сдвигала его внутри строки и уводила за собой все
    // следующие строки (`flex-gap-decorations-fragmentation-009/010`: из шести
    // элементов в колонках оставался один — v225 0.03 → v226 0.72); Blink растит
    // строку целиком (`flex_layout_algorithm.cc:2536-2560`
    // `item_offset_adjustment`). Спуск ВНУТРЬ элемента остаётся: распорка в его
    // потомке растит только его (`multi-line-row-flex-fragmentation-007`).
    // Только у контейнера ЗАДАННОЙ высоты: у `height: auto` в узкой колонке
    // строка обычно из одного элемента, и распорка перед ним — это распорка
    // перед строкой (`multi-line-row-flex-fragmentation-018/024/037`: без неё
    // «красное видно»).
    // И не для принудительного разрыва: распорка `break-before` перед
    // элементом — это и есть начало новой строки во фрагменте
    // (`multi-line-row-flex-fragmentation-029`).
    let wrap_flex = is_flex
        && c.style.flex_wrap == Some(true)
        && matches!(c.style.height, Some(Len::Px(_)))
        && !PUSH_FORCED.with(|f| f.get());
    // Та же гибкая стопка, что в `shape_full` (`flex_items`): поля не
    // схлопываются, между элементами `row-gap`, порядок визуальный,
    // внепоточные — не элементы.
    let flex_items = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.webkit_box != Some(true)
        && !row_nowrap;
    let flex_gap = match c.style.gap {
        Some((Some(Len::Px(v)), _)) if flex_items && c.style.vertical != Some(true) => v.max(0.0),
        _ => 0.0,
    };
    let mut items: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    if flex_items {
        items.sort_by_key(|n| match n {
            Node::Element(e) => e.style.order.unwrap_or(0),
            Node::Text(_) => 0,
        });
    }
    let mut y = top;
    let mut prev_mb = 0.0f32;
    let mut first = true;
    for n in items {
        let Node::Element(k) = n else {
            return None;
        };
        let oof = out_of_flow(&k.style);
        if flex_items && oof {
            continue;
        }
        let (h, kmt, kmb) = if oof {
            (0.0, 0.0, 0.0)
        } else if !k.inline
            && (k.style.position.is_none()
                || k.style.position == Some(crate::style::computed::Position::Relative))
            && k.style.float.unwrap_or(0) == 0
        {
            // Та же база элемента колонки flex, что у `shape_full`.
            let based = basis_sized(c, k);
            let (h, mt, mb, ..) = shape_full(based.as_ref().unwrap_or(k), depth - 1, ShapeCx::COLUMNS)?;
            (h, mt, mb)
        } else {
            return None;
        };
        if row_nowrap {
            // Элементы ряда стоят бок о бок с верха; точка внутри одного
            // из них — его собственная (объединение точек, `shape_full`).
            if !oof && a > top + 0.01 && a < top + h - 0.01 {
                if let Some(id) = pushed_box_at(k, a - top, depth - 1) {
                    return Some(id);
                }
            }
            continue;
        }
        // Та же арифметика, что в `shape_full`: у сетки поля не
        // схлопываются, между рядами — `row-gap`.
        let lead = if grid_rows_stack {
            if first {
                kmt
            } else {
                prev_mb + row_gap + kmt
            }
        } else if flex_items {
            if first { kmt } else { prev_mb + flex_gap + kmt }
        } else if first {
            if top == 0.0 {
                0.0
            } else {
                kmt
            }
        } else {
            prev_mb.max(kmt)
        };
        let start = y + lead;
        if !oof && (start - a).abs() < 0.01 {
            return (!wrap_flex).then_some(k.node_id);
        }
        // Принудительный разрыв гибкой стопки: `growths` отдаёт `nf` пары
        // `cuts` — начало ПОЛЯ элемента (конец зазора), а не верх коробки.
        if flex_items && !first && (start - kmt - a).abs() < 0.01 {
            return (!wrap_flex).then_some(k.node_id);
        }
        if !oof && start < a && a < start + h - 0.01 {
            return pushed_box_at(k, a - start, depth - 1);
        }
        y = start + h;
        prev_mb = kmb;
        first = false;
    }
    None
}

/// Распорка в таблице по тегу: та же арифметика рядов, что у `table_shape`
/// (порядок групп `thead`/…/`tfoot`, `border-spacing`, презентационные
/// `cellspacing`/`cellpadding`, высота ряда — наибольшая мера ячейки), без
/// `avoid` и разрывов; у ряда, накрывающего `a`, — первая ячейка, в которой
/// нашлась коробка. Точка ровно на верху ряда — первый ребёнок ячейки
/// (ячейка — свой контекст, поле сквозь её верх не уходит; в мере
/// `shape_full(cell)` распорка ложится в `lead`). Что `table_shape` не
/// меряет (`rowspan`, подпись, сросшиеся рамки, заданная высота), здесь
/// тоже `None` — распорки нет, поведение прежнее.
pub(crate) fn pushed_cell_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
    if depth == 0
        || c.style.vertical == Some(true)
        || c.style.border_collapse == Some(true)
        || c.style.height.is_some()
        || c.style.min_height.is_some()
    {
        return None;
    }
    let px_of = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let top = px_of(&c.style.padding.top)? + px_of(&c.style.borders().top)?;
    let attr_px = |name: &str| {
        c.attr(name)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
    };
    let ua_default = matches!(
        c.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let spacing = match (attr_px("cellspacing"), ua_default, &c.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => v,
        (_, _, Some((_, y))) => px_of(y)?,
        _ => 0.0,
    };
    let cell_cx = ShapeCx {
        cell_pad: attr_px("cellpadding"),
        ..ShapeCx::COLUMNS
    };
    let is_row = |e: &Element| e.tag == "tr" || e.style.display == Some(Display::TableRow);
    let is_group = |e: &Element| {
        matches!(e.tag.as_str(), "thead" | "tbody" | "tfoot")
            || e.style.display == Some(Display::TableRowGroup)
            || e.style.row_group_kind.is_some()
    };
    // Щуп разреза ходит по ТОМУ ЖЕ дереву, что мера (Х2) и рисование: иначе
    // `table_shape` посчитает точки по анонимным рядам, а `pushed_cell_at` на
    // тех же детях вернёт `None`, и распорка роста (`grow_pushed`) не найдёт
    // коробку, которую надо дотянуть до низа колонки.
    let fixed = fixup_table_children(&c.children);
    let mut parts: Vec<(u8, &Element)> = Vec::new();
    let (mut head, mut foot) = (false, false);
    for n in fixed.iter().filter(|n| !is_blank(n)) {
        let Node::Element(e) = n else { return None };
        let role = match e.tag.as_str() {
            "thead" => Some(0u8),
            "tbody" => Some(1),
            "tfoot" => Some(2),
            _ => e.style.row_group_kind,
        };
        let kind = match role {
            Some(0) if !head => {
                head = true;
                0
            }
            Some(2) if !foot => {
                foot = true;
                2
            }
            _ if is_row(e) || is_group(e) => 1,
            _ => return None,
        };
        parts.push((kind, e));
    }
    parts.sort_by_key(|p| p.0);
    let mut rows: Vec<&Element> = Vec::new();
    for (_, e) in &parts {
        if is_row(e) {
            rows.push(e);
            continue;
        }
        for n in e.children.iter().filter(|n| !is_blank(n)) {
            match n {
                Node::Element(r) if is_row(r) => rows.push(r),
                _ => return None,
            }
        }
    }
    let mut y = top;
    for r in rows {
        let start = y + spacing;
        let mut h = px_of(&r.style.height)?;
        let mut cells: Vec<&Element> = Vec::new();
        for n in r.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) || cell.attr("rowspan").is_some_and(|v| v.trim() != "1") {
                return None;
            }
            h = h.max(shape_full(cell, depth - 1, cell_cx)?.0);
            cells.push(cell);
        }
        if a > start - 0.01 && a < start + h - 0.01 {
            return cells
                .iter()
                .find_map(|cell| pushed_box_at(cell, a - start, depth - 1));
        }
        y = start + h;
    }
    None
}

/// Распорка роста: `margin-top += grow` у потомка `id`. Поле ложится в
/// `lead` меры (`shape_full`) и в раскладку копии одинаково; недобор от
/// схлопывания с большим нижним полем соседа добирает следующий заход
/// `grow_pushed`.
pub(crate) fn grow_before(c: &mut Element, id: u64, grow: f32) -> bool {
    for n in c.children.iter_mut() {
        let Node::Element(k) = n else {
            continue;
        };
        if k.node_id == id {
            let old = match &k.style.margin.top {
                Some(Len::Px(v)) => *v,
                _ => 0.0,
            };
            k.style.margin.top = Some(Len::Px(old + grow));
            return true;
        }
        if grow_before(k, id, grow) {
            return true;
        }
    }
    false
}

/// Распорка-КОРОБКА перед коробкой `id`: пустой блок высотой `grow`.
/// Нужна принудительному разрыву. Точка `forced` в мере стоит ПЕРЕД
/// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, потом
/// `forced.push(y)`), поэтому `margin-top` её не сдвигает — сдвигает только
/// новый поточный сосед.
pub(crate) fn spacer_before(c: &mut Element, id: u64, grow: f32) -> bool {
    // Сетка-стопка: между рядами стоит `row-gap` (`shape_full`:
    // `lead = prev_mb + row_gap + kmt`), и вставка ряда добавляет ЛИШНИЙ
    // зазор. Ряд flex без переноса: высота ряда — `tallest` по детям, а
    // распорка встала бы соседом БОК О БОК и подняла бы весь ряд. Оба
    // случая забирает подъём разрыва (Х5-Х8), а не рост.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row) | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // Зазор гибкой стопки (`row-gap`) встал бы и перед распоркой — лишний
    // зазор, как у сетки-стопки; такой разрыв остаётся без роста.
    let flex_gapped = is_flex
        && c.style.webkit_box != Some(true)
        && matches!(c.style.gap, Some((Some(Len::Px(v)), _)) if v > 0.0);
    let at = if grid_stack(c) || row_nowrap || flex_gapped {
        None
    } else {
        c.children
            .iter()
            .position(|n| matches!(n, Node::Element(k) if k.node_id == id))
    };
    if let Some(i) = at {
        // Распорка двигает только разрыв ПЕРЕД коробкой. Разрыв ПОСЛЕ
        // предыдущего соседа несёт `force_next`, и `forced.push(y)`
        // сработает на границе самой распорки: точка не сдвинется, а у
        // коробки исчезнет вовсе — распорка уедет в следующую колонку
        // вместе с ней. 11 пар из 51 в корзине D держатся только на
        // `break-after`; их забирает подъём (Х5-Х8).
        if !matches!(&c.children[i], Node::Element(k) if edge_break(k, false)) {
            return false;
        }
        // Нижнее поле предыдущего соседа переносится на распорку. Иначе
        // `lead` схлопывается ДВАЖДЫ — перед распоркой (`prev_mb.max(0)`) и
        // перед коробкой (`0.max(kmt)`), — и точка разрыва уезжает на
        // `prev_mb` НИЖЕ края колонки (`trailing-child-margin-000`, `-002`:
        // `margin-bottom: 50px`, обе зелёные).
        let pi = c.children[..i].iter().rposition(|n| !is_blank(n));
        let prev_mb = match pi.map(|j| &c.children[j]) {
            Some(Node::Element(k)) => k.style.margin.bottom.clone(),
            _ => None,
        };
        if prev_mb.is_some() {
            if let Some(Node::Element(k)) = pi.map(|j| &mut c.children[j]) {
                k.style.margin.bottom = None;
            }
        }
        let mut style = crate::style::computed::Computed::default();
        style.height = Some(Len::Px(grow));
        style.margin.bottom = prev_mb;
        // Распорка в гибком хозяине — сама элемент: при переносе по строкам
        // она обязана занять СВОЮ строку (иначе встаёт рядом с предыдущим
        // элементом и строку не растит), не сжиматься в контейнере с заданной
        // высотой и стоять в визуальном порядке рядом со своей коробкой
        // (`order`, `reorder` в `blocks()`).
        if matches!(c.style.display, Some(Display::Flex) | Some(Display::InlineFlex)) {
            style.width = Some(Len::Pct(1.0));
            style.flex_shrink = Some(0.0);
            if let Node::Element(k) = &c.children[i] {
                style.order = k.style.order;
            }
        }
        c.children.insert(
            i,
            Node::Element(Element {
                list_item: None,
                // Свой устойчивый номер: анимации у распорки нет, но номер
                // обязан быть уникальным — иначе GPUI склеит её состояние с
                // коробкой, перед которой она стоит.
                node_id: id ^ 0x5350_4143_4552_0001,
                anim: None,
                tag: "div".to_string(),
                style,
                hover: None,
                first_letter: None,
                first_line: None,
                children: Vec::new(),
                attrs: Vec::new(),
                inline: false,
            }),
        );
        // Заданная высота хозяина СТАРШЕ содержимого (`shape_full`: ветка
        // `c.style.height` возвращает `v + top + bot`), и распорка внутри неё
        // меры не меняет — `changed` не взводится, цикл `grow_pushed` встаёт
        // на первом заходе. Проба `p2-single-line-column-flex-fragmentation-
        // 037` осталась красной именно поэтому, а `p3-…` с высотой 100 → 150
        // сняла 4/5 площади (3906 → 756 точек).
        if let Some(Len::Px(h)) = c.style.height {
            c.style.height = Some(Len::Px(h + grow));
        }
        return true;
    }
    for n in c.children.iter_mut() {
        if let Node::Element(k) = n {
            if spacer_before(k, id, grow) {
                return true;
            }
        }
    }
    false
}

/// Рост коробки от вытолкнутого монолита (Blink `FinishFragmentation`,
/// `fragmentation_utils.cc:641-656`: у НЕпоследнего фрагмента
/// `final_block_size = space_left`; css-flexbox-1 §fragmentation: «A forced
/// break inside a flex item effectively increases the size of its
/// contents»). Укладка режет ребёнка стопки в НАЧАЛЕ монолитного диапазона
/// `a` (`flow.rs` `fill_at`: `holds` → `at(a)`): в колонке остаётся
/// `a − from` содержимого при остатке `room`, и коробки, где лежит монолит,
/// обязаны дотянуться до низа, а всё после него — сдвинуться на
/// `grow = room − (a − from)`. Копия одна на все колонки, и сдвиг в ней
/// делает распорка — `margin-top += grow` у самой внешней коробки,
/// начинающейся ровно в `a` (`pushed_box_at`). После неё монолит стоит на
/// краю колонки, мера выросла на `grow`, и срез по краю совпадает с Blink:
/// колонка 1 — `room`, следующая копия — с `from + room`. План зависит
/// только от мер (`ColumnStack::growths`), поэтому распорки ставятся ДО
/// сборки копий, до неподвижной точки (≤ 6 заходов; обычно один). Проба
/// устройства руками — `target/probe-9d/*.html`: SLC-007/009/010/011 и
/// `table-cell-expansion-001` 0.00 (`scout-break-2026-09d.md` §3.3).
pub(crate) fn grow_pushed(
    mut kids: Vec<(Element, Shape)>,
    count: usize,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    par: &[crate::layout::fragment::types::Par],
) -> Vec<(Element, Shape)> {
    for _ in 0..6 {
        let probe: Vec<crate::layout::fragment::types::Kid> = kids
            .iter()
            .enumerate()
            .map(|(i, (c, s))| crate::layout::fragment::types::Kid {
                h: s.0,
                mt: s.1,
                mb: s.2,
                // Тот же предикат, что у `StackChild` в сборке стопки.
                monolith: solid_box(c),
                cuts: s.3.clone(),
                // Щуп обязан видеть ровно то же, что стопка (Х6), иначе
                // распорки лягут по другому плану, чем укладка.
                force_before: edge_break(c, false),
                force_after: edge_break(c, true),
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // запретов `growths` считал бы распорки по ДРУГОМУ плану,
                // чем укладка после отступа.
                avoid_before: edge_avoid(c, false),
                avoid_after: edge_avoid(c, true),
                forced: s.4.clone(),
                solid: s.5.clone(),
                span: c.style.column_span == Some(true) && !c.inline,
                // Щуп роста параллельного потока не знает: распорка
                // (`spacer_before`) — про в-поточную высоту, а поток высоты
                // не даёт (css-break-3 §3). Раздвинуть коробку им значило бы
                // вернуть переполнение в поток. `over == 0.0` при `h >= 0`
                // выключает поток в `fill_at` тождественно.
                over: 0.0,
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // `clone` соседи после такой коробки получили бы распорки по
                // ДРУГОМУ плану.
                clone_dec: clone_dec(c),
                // Тот же предикат, что у `StackChild` в сборке стопки: иначе
                // распорки легли бы по другому плану, чем укладка.
                overflow_top: fixed.is_some() && rows.is_none() && !parallel_items_inside(c, 4),
                repeat: repeat_leads(c, fixed, rows),
                par: par.get(i).copied().unwrap_or_default(),
            })
            .collect();
        let mut grows = crate::layout::multicol::column_stack::ColumnStack::growths(&probe, count, fixed, rows, copies);
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает точки
        // выше, и `at` из одного плана остаётся верным для всех записей
        // прохода (рост дорожки иначе находил ряд по устаревшим полосам:
        // `grid-item-fragmentation-048`, два разрыва в одном проходе).
        grows.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal))
        });
        let mut changed = false;
        for (kid, at, grow, forced) in grows {
            let c = &mut kids[kid].0;
            // Ряд в точках, вытолкнутый целиком: растёт ПРЕДЫДУЩАЯ дорожка.
            if grow_grid_track(c, at, grow) {
                if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
                    kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                    changed = true;
                }
                continue;
            }
            PUSH_FORCED.with(|f| f.set(forced));
            let found = pushed_box_at(c, at, 4);
            PUSH_FORCED.with(|f| f.set(false));
            let Some(id) = found else {
                continue;
            };
            // Монолит двигает поле, принудительный разрыв — коробка (Х3).
            let moved = if forced {
                spacer_before(c, id, grow)
            } else {
                grow_before(c, id, grow)
            };
            if !moved {
                continue;
            }
            if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
                kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    kids
}

/// Монолит ребёнка колонок держится ТОЛЬКО на `break-inside: avoid`: ни
/// `contain: size`, ни прокрутки, ни замещаемого, ни атомарной строчной, ни
/// сплошного строчного набора (тот же список, что у `monolith` в сборке
/// стопки, без `break_inside_avoid`).
pub(crate) fn avoid_only_monolith(c: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| matches!(o, Some(crate::style::computed::Overflow::Scroll));
    let block_kid = |n: &Node| matches!(n, Node::Element(k) if !k.inline || k.style.display == Some(Display::Block));
    !(size_monolith(c)
        || scrolls(c.style.overflow_x)
        || scrolls(c.style.overflow_y)
        || matches!(
            c.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            c.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
        || (c.children.iter().any(|n| !is_blank(n)) && !c.children.iter().any(block_kid)))
}

/// Перемера после распорки — с прежними полями у элемента строки flex
/// (`split_flex_lines` кладёт в поле ещё и `row-gap`).
pub(crate) fn keep_par_margins(s: Shape, old: &Shape, par: Option<&crate::layout::fragment::types::Par>) -> Shape {
    if par.is_some_and(|p| p.group != 0) {
        (s.0, old.1, old.2, s.3, s.4, s.5)
    } else {
        s
    }
}
