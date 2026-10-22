//! Сбор блочной стопки копии из дерева: какие коробки стопка видит (`stacked_tree`), их
//! свойства по стилю (`collect`) и раскрытие строчной коробки с блоками внутри.

use super::{BlockBox, BlockKid, grid, grid_container};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flex_lines::class_a_box;
use crate::layout::page::names::page_names;
use crate::style::computed::{Display, LenCx};

/// Блочный контейнер, чьи дети идут стопкой по блочной оси: блок и элемент
/// списка. Flex, сетка, таблица, многоколоночник и строчный набор кладут
/// детей иначе — их границы дают свои контексты (строки — `Paragraph`).
pub(super) fn stacks_blocks(e: &Element) -> bool {
    matches!(
        e.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) && e.style.webkit_box != Some(true)
        && !crate::layout::multicol::spanner::multicol_container(&e.style)
        && !crate::layout::fragment::table_bands::table_box(e)
        && !matches!(e.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
}

/// Всё поточное блочное поддерево `e` до глубины `depth` — блочные стопки и
/// монолиты: каждую коробку, перед которой может встать распорка, рекордер
/// стопки видит (таблица, flex и сетка кладут детей иначе).
pub(crate) fn stacked_tree(e: &Element, depth: u8) -> bool {
    stacks_blocks(e)
        && (depth == 0
            || e.children.iter().all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style) || k.inline => {
                    !holds_blocks(k) || stacked_tree(k, depth - 1)
                }
                Node::Element(k) => monolith_box(k) || stacked_tree(k, depth - 1),
                _ => true,
            }))
}

/// `blockified` — элемент сетки: его `inline-block` — блок (css-display-3 §2.7).
pub(super) fn collect(
    e: &Element,
    depth: u8,
    blockified: bool,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> BlockBox {
    let lens = LenCx::box_viewport(cx.viewport, &e.style);
    let px = |l| lens.px_lossy_zero(l, vcx);
    let b = e.style.borders();
    let mut out = BlockBox {
        id: e.node_id,
        head: px(e.style.padding.top) + px(b.top),
        tail: px(e.style.padding.bottom) + px(b.bottom),
        end_margin: crate::layout::box_kind::bfc_root(e)
            && !grid_container(e)
            && matches!(
                e.style.height,
                None | Some(crate::style::values::value::Len::Auto)
            ),
        kids: Vec::new(),
        grid: None,
    };
    if depth == 0 {
        return out;
    }
    if grid_container(e) {
        out.grid = Some(grid::collect_grid(e, depth - 1, cx, vcx));
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

/// Поточные блочные дети из узлов `nodes`. Блок внутри строчной коробки
/// разрывает её (CSS 2.1 §9.2.1.1): части строчной коробки уходят в
/// анонимные блоки, а сам блок — сосед блоков того же потока, поэтому
/// строчная коробка с блоками внутри раскрывается на месте.
fn push_kids(
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
            inner: (!monolith && (stacks_blocks(k) || grid_container(k)))
                .then(|| collect(k, depth - 1, false, cx, vcx)),
            page: (cx.paged && class_a_box(k)).then(|| page_names(k, "")),
        });
        *adjoins = true;
    }
}

/// Строчная коробка с поточным блоком внутри (на любой глубине строчных).
fn holds_blocks(e: &Element) -> bool {
    e.children.iter().any(|n| match n {
        Node::Element(k) if out_of_flow(&k.style) => false,
        Node::Element(k) if !k.inline => true,
        Node::Element(k) => k.style.display.is_none() && holds_blocks(k),
        _ => false,
    })
}
