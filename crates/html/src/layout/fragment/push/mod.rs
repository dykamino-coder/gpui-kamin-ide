//! ADR P-19 TRANSITIONAL(F2-1/F2-3/F2-4): оценочный рост остаётся у preplan; measured использует геометрию копии.
//! Перенос коробок на следующий фрагмент распорками роста (`grow_pushed`): по точке разреза
//! `a` от верха ребёнка стопки ищется самая внешняя коробка, начинающаяся ровно там
//! (`pushed_box_at`), и перед ней встаёт распорка. Смещения детей считаются той же
//! арифметикой, что в `shape_full`: схлопнутые поля, зазоры flex и сетки, ряды таблицы
//! (`cell`), строки flex-ряда (`row`); текст и строчное среди детей — отказ от спуска.
//! Подмодуль `spacer` — сама распорка и монолит `break-inside: avoid`.

use crate::document::state::ValueCx;

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{grid_auto_row_bands, grid_items_spotted, grid_stack};
use crate::layout::fragment::line_shape::basis_sized;
use crate::layout::fragment::table_bands::table_box;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod cell;
mod row;
use cell::pushed_cell_at;
mod spacer;
pub(crate) use spacer::avoid_only_monolith;
pub(crate) use spacer::grow_pushed;
pub(crate) mod flex_flow;
mod grid_row;
mod measured;
mod vertical;
pub(crate) use measured::{Growth, Regrow, measured_push, measured_tree};

// Самая внешняя коробка, начинающаяся ровно в `a` от верха `c`, — перед
// ней встаёт распорка роста (`grow_pushed`). Смещения детей — той же
// арифметикой, что в `shape_full`: `lead` = схлопнутое поле, у первого
// без отбивки поле уходит сквозь верх, внепоточный — нулевая запись, ряд
// flex без переноса — все дети с верха. Сетка, таблица, ряды — без
// спуска, как там; таблица по тегу — рядами `table_shape`
// (`pushed_cell_at`). Текст или строчный среди детей — у `shape_full`
// отказ от спуска, и здесь тоже.
// `forced` — щуп ищет коробку под ПРИНУДИТЕЛЬНЫЙ разрыв (`grow_pushed`).
fn pushed_box_at(c: &Element, a: f32, depth: u8, forced: bool, vcx: ValueCx<'_>) -> Option<u64> {
    if depth == 0 {
        return None;
    }
    let lines = crate::layout::fragment::LineCx::child(c, vcx);
    let vcx = vcx.with_lines(lines.as_ref().or(vcx.lines));
    if let Some(found) = row::pushed_row_at(c, a, depth, forced, vcx) {
        return found;
    }
    if table_box(c) {
        return pushed_cell_at(c, a, depth, forced, vcx);
    }
    // Сетка-стопка спускается ровно как в `shape_full` — иначе распорка
    // роста до ряда сетки не добирается, отдаёт `None`, и фон
    // коробки не дотягивается до низа колонки.
    let grid_rows_stack = grid_stack(c);
    let row_gap = if grid_rows_stack {
        match c.style.used().gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    if (matches!(
        c.style.used().display,
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
    let px = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
    let b = c.style.used().borders();
    let top = px(&c.style.used().padding.top) + px(&b.top);
    // Сетка `grid_auto_row_bands`: коробка ищется в элементах ряда,
    // накрывающего `a`, — теми же смещениями, что в мере (`shape_full`).
    // Точка ровно на верху элемента уходит в его первого ребёнка.
    if grid_items_spotted(c) {
        let (bands, spots) = grid_auto_row_bands(c, depth, ShapeCx::COLUMNS, vcx)?;
        for (ix, row, kmt, s, plain) in spots {
            let (true, Some(&(r0, _)), Some(Node::Element(k))) =
                (plain, bands.get(row), c.children.get(ix))
            else {
                continue;
            };
            let start = top + r0 + kmt;
            if a > start - 0.01
                && a < start + s.height - 0.01
                && let Some(id) = pushed_box_at(k, a - start, depth - 1, forced, vcx)
            {
                return Some(id);
            }
        }
        return None;
    }
    let is_flex = matches!(
        c.style.used().display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.used().webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.used().flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.used().flex_wrap != Some(true)
        && c.style.cascaded().webkit_box_vertical != Some(true);
    // МНОГОСТРОЧНЫЙ flex: элемент — не коробка стопки, а член СТРОКИ. Распорка
    // перед самим элементом сдвигала его внутри строки и уводила за собой все
    // следующие строки (`flex-gap-decorations-fragmentation-009/010`: из шести
    // элементов в колонках оставался один); Blink растит
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
        && c.style.used().flex_wrap == Some(true)
        && matches!(c.style.used().height, Some(Len::Px(_)))
        && !forced;
    // Та же гибкая стопка, что в `shape_full` (`flex_items`): поля не
    // схлопываются, между элементами `row-gap`, порядок визуальный,
    // внепоточные — не элементы.
    let flex_items = matches!(
        c.style.used().display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && c.style.used().webkit_box != Some(true)
        && !row_nowrap;
    let flex_gap = match c.style.used().gap {
        Some((Some(Len::Px(v)), _)) if flex_items && !c.style.cascaded().wm().vertical() => {
            v.max(0.0)
        }
        _ => 0.0,
    };
    let mut items: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    if flex_items {
        items.sort_by_key(|n| match n {
            Node::Element(e) => e.style.used().order.unwrap_or(0),
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
        let oof = out_of_flow(&k.style.used());
        if flex_items && oof {
            continue;
        }
        let (h, kmt, kmb) = if oof {
            (0.0, 0.0, 0.0)
        } else if !k.inline
            && (k.style.used().position.is_none()
                || k.style.used().position == Some(crate::style::computed::Position::Relative))
            && k.style.used().float.unwrap_or(0) == 0
        {
            // Та же база элемента колонки flex, что у `shape_full`.
            let based = basis_sized(c, k);
            let crate::layout::fragment::Shape {
                height: h,
                margin_top: mt,
                margin_bottom: mb,
                ..
            } = shape_full(
                based.as_ref().unwrap_or(k),
                depth - 1,
                ShapeCx::COLUMNS,
                vcx,
            )?;
            (h, mt, mb)
        } else {
            return None;
        };
        if row_nowrap {
            // Элементы ряда стоят бок о бок с верха; точка внутри одного
            // из них — его собственная (объединение точек, `shape_full`).
            if !oof
                && a > top + 0.01
                && a < top + h - 0.01
                && let Some(id) = pushed_box_at(k, a - top, depth - 1, forced, vcx)
            {
                return Some(id);
            }
            continue;
        }
        // Та же арифметика, что в `shape_full`: у сетки поля не
        // схлопываются, между рядами — `row-gap`.
        let lead = if grid_rows_stack {
            if first { kmt } else { prev_mb + row_gap + kmt }
        } else if flex_items {
            if first { kmt } else { prev_mb + flex_gap + kmt }
        } else if first {
            if top == 0.0 { 0.0 } else { kmt }
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
            return pushed_box_at(k, a - start, depth - 1, forced, vcx);
        }
        y = start + h;
        prev_mb = kmb;
        first = false;
    }
    None
}
