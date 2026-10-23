//! Сбор блочной стопки копии из дерева по стилю: коробка с рамкой и отбивкой у краёв
//! (`frame`), таблица (`table::TableBox`), сетка (`grid::collect_grid`) и поточные блочные
//! дети с раскрытием строчной коробки с блоками внутри (`push_kids`).

use super::table::TableBox;
use super::trees::{holds_blocks, stacks_blocks};
use super::{BlockBox, BlockKid, flex, flex_container, grid, grid_container};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flex_lines::class_a_box;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::page::names::page_names;
use crate::style::computed::{Display, LenCx};

/// `blockified` — элемент сетки: его `inline-block` — блок (css-display-3 §2.7).
pub(in crate::layout::fragment::flow::blocks) fn collect(
    e: &Element,
    depth: u8,
    blockified: bool,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> BlockBox {
    // Таблица горизонтального письма: обёртка без своих рамки и отбивки,
    // её границы — подписи и ряды (`table`). Вертикальную таблицу рекордер
    // не читает (блочные границы — только по горизонтальной оси).
    if table_box(e) && !e.style.wm().vertical() {
        return BlockBox {
            id: e.node_id,
            table: (depth > 0).then(|| Box::new(TableBox::collect(e, depth, cx, vcx))),
            ..Default::default()
        };
    }
    let mut out = frame(e, cx, vcx);
    if depth == 0 {
        return out;
    }
    if grid_container(e) {
        out.grid = Some(grid::collect_grid(e, depth - 1, cx, vcx));
        return out;
    }
    if flex_container(e) {
        out.flex = Some(flex::collect_flex(e, depth - 1, cx, vcx));
        return out;
    }
    let stacks = stacks_blocks(e) || (blockified && e.style.display == Some(Display::InlineBlock));
    if !stacks {
        return out;
    }
    let mut adjoins = false;
    push_kids(&e.children, &mut out.kids, &mut adjoins, depth, cx, vcx);
    out
}

/// Коробка `e` без детей: рамка с отбивкой у краёв и корень контекста.
pub(super) fn frame(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> BlockBox {
    let lens = LenCx::box_viewport(cx.viewport, &e.style);
    let px = |l| lens.px_lossy_zero(l, vcx);
    let b = e.style.borders();
    BlockBox {
        id: e.node_id,
        own_breaks: (
            e.style.break_before_force,
            e.style.break_after_force,
            e.style.break_before_avoid,
            e.style.break_after_avoid,
        ),
        inside_avoid: e.style.break_inside_avoid,
        edge_kids: edges(e),
        head: px(e.style.padding.top) + px(b.top),
        tail: px(e.style.padding.bottom) + px(b.bottom),
        end_margin: crate::layout::box_kind::bfc_root(e)
            && !grid_container(e)
            && !flex_container(e)
            && matches!(
                e.style.height,
                None | Some(crate::style::values::value::Len::Auto)
            ),
        kids: Vec::new(),
        table: None,
        grid: None,
        flex: None,
        flex_gap: None,
    }
}

/// Поточные блочные дети из узлов `nodes`. Блок внутри строчной коробки
/// разрывает её (CSS 2.1 §9.2.1.1): части строчной коробки уходят в
/// анонимные блоки, а сам блок — сосед блоков того же потока, поэтому
/// строчная коробка с блоками внутри раскрывается на месте.
pub(super) fn push_kids(
    nodes: &[Node],
    out: &mut Vec<BlockKid>,
    adjoins: &mut bool,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) {
    for n in nodes.iter().filter(|n| !is_blank(n)) {
        let k = match n {
            // Внепоточные соседей не двигают (CSS 2.1 §9.3.1) и границу
            // соседей не прерывают.
            Node::Element(k) if out_of_flow(&k.style) => continue,
            Node::Element(k) if matches!(k.style.display, Some(Display::None)) => continue,
            Node::Element(k) if !k.inline && k.style.display != Some(Display::Contents) => k,
            Node::Element(k) if k.inline && k.style.display.is_none() && holds_blocks(k) => {
                push_kids(&k.children, out, adjoins, depth, cx, vcx);
                continue;
            }
            // Строчное содержимое между блоками — анонимный блок строк: его
            // границы дают измеренные строки, не эта стопка.
            _ => {
                *adjoins = false;
                continue;
            }
        };
        let monolith = monolith_box(k);
        out.push(BlockKid {
            id: k.node_id,
            adjoins: *adjoins,
            force_before: edge_break(k, false, vcx),
            force_after: edge_break(k, true, vcx),
            avoid_before: edge_avoid(k, false, vcx),
            avoid_after: edge_avoid(k, true, vcx),
            monolith,
            inner: (!monolith
                && (stacks_blocks(k) || table_box(k) || grid_container(k) || flex_container(k)))
            .then(|| collect(k, depth - 1, false, cx, vcx)),
            page: (cx.paged && class_a_box(k)).then(|| page_names(k, "")),
        });
        *adjoins = true;
    }
}

/// Крайние поточные узлы; текст и строчная коробка не передают блочный break.
fn edges(e: &Element) -> (Option<u64>, Option<u64>) {
    let nodes: Vec<_> = e.children.iter().filter(|n| !is_blank(n)).filter(|n| {
        !matches!(n, Node::Element(k) if out_of_flow(&k.style) || k.style.display == Some(Display::None))
    }).collect();
    let block = |n: &&Node| match n {
        Node::Element(k) if !k.inline => Some(k.node_id),
        _ => None,
    };
    (nodes.first().and_then(block), nodes.last().and_then(block))
}
