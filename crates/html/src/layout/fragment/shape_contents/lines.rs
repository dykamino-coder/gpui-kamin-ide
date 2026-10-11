//! Признаки строчных детей и оси рядов (flex/grid) для формы содержимого.

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::render::out_of_flow;
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn line_inner_kids(c: &Element, kids: &Vec<&Node>, avoid_chains: bool) -> Vec<bool> {
    let main_gap0 = match c.style.gap {
        None | Some((_, None)) => true,
        Some((_, Some(Len::Px(v)))) => v.abs() < 0.01,
        _ => false,
    };
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let pct_of = |k: &Element| -> Option<f32> {
        let kb = k.style.borders();
        match k.style.width {
            Some(Len::Pct(p))
                if main_gap0
                    && k.style.flex_basis.is_none()
                    && k.style.min_width.is_none()
                    && k.style.max_width.is_none()
                    && zero(&k.style.margin.left)
                    && zero(&k.style.margin.right)
                    && zero(&k.style.padding.left)
                    && zero(&k.style.padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right) =>
            {
                Some(p)
            }
            _ => None,
        }
    };
    let mut line_acc: Option<f32> = None;
    let mut line_fa = false;
    let line_inner: Vec<bool> = kids
        .iter()
        .map(|n| match n {
            // Внепоточный — не элемент (§4.1): строку не рвёт и в неё не входит.
            Node::Element(k) if avoid_chains && out_of_flow(&k.style) => false,
            Node::Element(k) if avoid_chains => {
                let w = pct_of(k);
                let forced = line_fa || edge_break(k, false);
                line_fa = edge_break(k, true);
                let inner =
                    !forced && matches!((line_acc, w), (Some(s), Some(p)) if s + p <= 1.0 + 1e-3);
                line_acc = match w {
                    Some(p) if inner => line_acc.map(|s| s + p),
                    w => w,
                };
                inner
            }
            _ => {
                line_acc = None;
                line_fa = false;
                false
            }
        })
        .collect();
    line_inner
}

pub(super) fn row_axis_flags(c: &Element) -> (bool, bool, f32, bool) {
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
    // Сетка в одну колонку с рядами по содержимому — стопка (`grid_stack`):
    // ряд равен одному ребёнку. Без спуска её мера уходила в `grid_rows_px`
    // и почти всегда возвращала `None`, а `None` означает отказ от укладки:
    // многоколоночник с такой сеткой внутри не фрагментировался ВОВСЕ —
    // колонка 1 переполнена, остальные пусты (`scout-break-2026-09f.md` §1;
    // пробы `target/probe-9f/p-grid-item-fragmentation-043.html` и
    // `p-grid-container-fragmentation-009.html` = 0.00 при подмене на блок).
    let grid_rows_stack = grid_stack(c);
    // Зазор рядов такой сетки: `grid_stack` ручается, что он в точках.
    let row_gap = if grid_rows_stack {
        match c.style.gap {
            Some((Some(Len::Px(v)), _)) => v,
            _ => 0.0,
        }
    } else {
        0.0
    };
    // Ряд/группа рядов ВНЕ таблицы (тегом или `display`) — не стопка блоков.
    let no_descent = (matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    ) && !grid_rows_stack)
        || matches!(c.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot");
    (row_nowrap, grid_rows_stack, row_gap, no_descent)
}
