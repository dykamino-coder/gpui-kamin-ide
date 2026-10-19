//! Subgrid flatten entry phase. CSS Grid 2 §9.

use super::super::types::GridItem;
use super::super::types::NamedLineResolver;
use super::{flatten_into, linked_axes, SubgridEdge};
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::util::sys::Vec;
use crate::LayoutGridContainer;

/// Сплющить элементы подсеток-детей в дорожки контейнера (css-grid-2 §9 (h)).
///
/// `items` — элементы самого контейнера (после размещения); в конец
/// дописываются сплющенные элементы, а у самой подсетки снимается бит
/// подсеточной оси («acts as if it was completely empty for track sizing
/// purposes in the subgridded dimension», §9 (g)). `container_gap` —
/// использованные зазоры контейнера (точки). `axis_mask` — оси, по которым
/// контейнер вообще связывает подсетки (у лунок — только ось решётки,
/// css-grid-3 Overview.bs:502-519). Возвращает края всех подсеток дерева —
/// для пола дорожек (`apply_subgrid_floors`).
pub(in super::super) fn flatten_subgrid_items<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    items: &mut Vec<GridItem>,
    container_gap: Size<f32>,
    inner_node_size: Size<Option<f32>>,
    axis_mask: u8,
    resolver: Option<&NamedLineResolver<Tree::CustomIdent>>,
    root_axes: Size<bool>,
) -> Vec<SubgridEdge> {
    let own = items.len();
    let mut out: Vec<GridItem> = Vec::new();
    let mut edges: Vec<SubgridEdge> = Vec::new();
    for item in items.iter_mut().take(own) {
        let linked = linked_axes(tree, item.node) & axis_mask;
        if linked == 0 {
            continue;
        }
        item.sizing_axes &= !linked;
        let area = (item.column, item.row);
        let (first_edge, first_item) = (edges.len(), out.len());
        let known = super::stack_constraint::root_content(tree, item, inner_node_size, axis_mask);
        flatten_into(
            tree,
            item.node,
            linked,
            area,
            area,
            root_axes,
            root_axes,
            resolver,
            Rect::ZERO,
            container_gap,
            container_gap,
            inner_node_size,
            known,
            &mut out,
            &mut edges,
            0,
        );
        for edge in edges.iter_mut().skip(first_edge) {
            edge.root = item.node;
        }
        for flat in out.iter_mut().skip(first_item) {
            flat.subgrid_root = Some(item.node);
        }
    }
    items.extend(out);
    edges
}
