//! Перенос коробок на следующий фрагмент.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{grid_auto_row_bands, grid_items_spotted, grid_stack};
use crate::layout::fragment::line_shape::basis_sized;
use crate::layout::fragment::table_bands::table_box;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod cell;
use cell::pushed_cell_at;
mod spacer;
pub(crate) use spacer::avoid_only_monolith;
pub(crate) use spacer::grow_pushed;

// Самая внешняя коробка, начинающаяся ровно в `a` от верха `c`, — перед
// ней встаёт распорка роста (`grow_pushed`). Смещения детей — той же
// арифметикой, что в `shape_full`: `lead` = схлопнутое поле, у первого
// без отбивки поле уходит сквозь верх, внепоточный — нулевая запись, ряд
// flex без переноса — все дети с верха. Сетка, таблица, ряды — без
// спуска, как там; таблица по тегу — рядами `table_shape`
// (`pushed_cell_at`). Текст или строчный среди детей — у `shape_full`
// отказ от спуска, и здесь тоже.
thread_local! {
    /// Щуп `pushed_box_at` ищет коробку под ПРИНУДИТЕЛЬНЫЙ разрыв (`grow_pushed`).
    static PUSH_FORCED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn pushed_box_at(c: &Element, a: f32, depth: u8) -> Option<u64> {
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
            if a > start - 0.01
                && a < start + s.0 - 0.01
                && let Some(id) = pushed_box_at(k, a - start, depth - 1)
            {
                return Some(id);
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
            None | Some(crate::style::computed::FlexDir::Row)
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
            let (h, mt, mb, ..) =
                shape_full(based.as_ref().unwrap_or(k), depth - 1, ShapeCx::COLUMNS)?;
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
                && let Some(id) = pushed_box_at(k, a - top, depth - 1)
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
            return pushed_box_at(k, a - start, depth - 1);
        }
        y = start + h;
        prev_mb = kmb;
        first = false;
    }
    None
}
