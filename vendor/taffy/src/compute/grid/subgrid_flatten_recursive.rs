//! Subgrid flatten recursive phase. CSS Grid 2 §9.

use super::super::explicit_grid::AutoRepeatStrategy;
use super::super::types::GridItem;
use super::super::types::NamedLineResolver;
use super::super::OriginZeroLine;
use super::{
    linked_axes, nested_standalone_size, place_items, subgrid_edges, subgrid_gaps, PlacedGrid,
    SubgridEdge, MAX_DEPTH,
};
use crate::geometry::Line;
use crate::geometry::Rect;
use crate::geometry::Size;
use crate::style::AlignItemsKeyword;
use crate::style::SUBGRID_COLUMNS;
use crate::style::SUBGRID_ROWS;
use crate::tree::NodeId;
use crate::util::sys::Vec;
use crate::CoreStyle;
use crate::GridContainerStyle;
use crate::LayoutGridContainer;

/// Один уровень сплющивания: элементы подсетки `node`, чья область в
/// координатах КОРНЕВОЙ сетки — `area`, а накопленный «extra margin» самой
/// подсетки — `ext`.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn flatten_into<Tree: LayoutGridContainer>(
    tree: &mut Tree,
    node: NodeId,
    linked: u8,
    area: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    local: (Line<OriginZeroLine>, Line<OriginZeroLine>),
    root_axes: Size<bool>,
    parent_axes: Size<bool>,
    parent_resolver: Option<&NamedLineResolver<Tree::CustomIdent>>,
    ext: Rect<f32>,
    parent_gap: Size<f32>,
    root_gap: Size<f32>,
    inner_node_size: Size<Option<f32>>,
    known_content: Size<Option<f32>>,
    out: &mut Vec<GridItem>,
    edges_out: &mut Vec<SubgridEdge>,
    depth: u8,
) {
    if depth >= MAX_DEPTH {
        return;
    }
    let style = tree.get_grid_container_style(node);
    let bits = style.subgrid();
    let own_axes = super::super::subgrid_flow::axes(&style);
    let edges = subgrid_edges(tree, &style, inner_node_size.width);
    let gap = subgrid_gaps(tree, &style, bits, parent_gap, inner_node_size);
    let sub = super::setup::tracks(
        linked,
        area,
        local,
        gap,
        parent_resolver,
        own_axes,
        parent_axes,
    );
    // Upstream #1254: `align-items`/`justify-items` are no longer `Option` (default `normal`).
    let align_items = style.align_items();
    let justify_items = style.justify_items();
    // Размер для `repeat(auto-*)` в СВОЕЙ оси подсетки здесь неизвестен —
    // повтор идёт один раз (css-grid-2 §7.2.3.2 «Otherwise, the specified
    // track list repeats only once»). Подсеточная ось повторов не знает.
    let strategy = Size {
        width: AutoRepeatStrategy::MinRepetitionsThatDoOverflow,
        height: AutoRepeatStrategy::MinRepetitionsThatDoOverflow,
    };
    let placed = place_items(
        tree,
        node,
        &style,
        Size::NONE,
        strategy,
        Some(&sub),
        align_items,
        justify_items,
    );
    drop(style);
    super::setup::record_edges(
        linked, area, root_axes, ext, edges, gap, root_gap, node, edges_out,
    );

    // Собственные дорожки НЕподсеточной оси, если они известны без
    // содержимого (`standalone_tracks`): по ним сплющенный элемент меряется
    // поперёк вместо всей области подсетки.
    let style = tree.get_grid_container_style(node);
    let own_cols = (linked & SUBGRID_COLUMNS == 0)
        .then(|| {
            super::cross_constraints::own_tracks(
                tree,
                &style,
                true,
                inner_node_size,
                known_content,
                &placed.items,
            )
        })
        .flatten();
    let own_rows = (linked & SUBGRID_ROWS == 0)
        .then(|| {
            super::cross_constraints::own_tracks(
                tree,
                &style,
                false,
                inner_node_size,
                known_content,
                &placed.items,
            )
        })
        .flatten();
    // KaminIDE patch: см. `GridItem::subgrid_cross_auto` — у НЕподсеточной
    // оси без шаблона и без `grid-auto-*` дорожки неявные `auto`.
    let implicit_auto = |columns: bool| {
        let (template, auto) = if columns {
            (
                style.grid_template_columns().map_or(true, |t| t.len() == 0),
                style.grid_auto_columns().len() == 0,
            )
        } else {
            (
                style.grid_template_rows().map_or(true, |t| t.len() == 0),
                style.grid_auto_rows().len() == 0,
            )
        };
        template && auto
    };
    let cross_auto = Size {
        width: linked & SUBGRID_COLUMNS == 0 && own_cols.is_none() && implicit_auto(true),
        height: false,
    };
    drop(style);
    let PlacedGrid {
        items: placed_items,
        name_resolver: own_resolver,
        ..
    } = placed;
    for mut child in placed_items {
        let chained = linked_axes(tree, child.node) & linked;
        let column = if linked & SUBGRID_COLUMNS != 0 {
            super::super::subgrid_flow::project(
                child.column,
                area.0,
                own_axes.width != root_axes.width,
            )
        } else {
            area.0
        };
        let row = if linked & SUBGRID_ROWS != 0 {
            super::super::subgrid_flow::project(
                child.row,
                area.1,
                own_axes.height != root_axes.height,
            )
        } else {
            area.1
        };
        // §subgrid-margins: у крайних элементов — края подсетки поверх
        // накопленного; §subgrid-gaps: у внутренних сторон — половина разницы
        // зазоров. Разница телескопична через уровни: внутренняя линия
        // вложенной подсетки — внутренняя и у внешней, и итог равен половине
        // разницы СВОЕГО зазора и зазора корневой сетки.
        //
        // В НЕподсеточной оси элемент лежит в собственных дорожках подсетки, а
        // сплющенному известна лишь вся её область: края подсетки с обеих
        // сторон сужают эту область до коробки содержимого — по ней элемент
        // и меряется поперёк (перенос строк при растяжке). В размер дорожек
        // этой оси он не вкладывается (`sizing_axes`).
        let mut extra =
            super::item_edges::resolve(ext, edges, gap, root_gap, linked, &child, area, own_axes);
        child.subgrid_cross_auto = cross_auto;
        if let Some(width) = nested_standalone_size::cross(&own_cols, child.column) {
            child.subgrid_cross.width = Some(width);
            extra.left = 0.0;
            extra.right = 0.0;
        }
        if let Some(height) = nested_standalone_size::cross(&own_rows, child.row) {
            child.subgrid_cross.height = Some(height);
            extra.top = 0.0;
            extra.bottom = 0.0;
        }
        if chained != 0 {
            let child_content = nested_standalone_size::content(tree, &child, chained);
            flatten_into(
                tree,
                child.node,
                chained,
                (column, row),
                (child.column, child.row),
                root_axes,
                own_axes,
                Some(&own_resolver),
                extra,
                gap,
                root_gap,
                inner_node_size,
                child_content,
                out,
                edges_out,
                depth + 1,
            );
        }
        let sizing = linked & !chained;
        if sizing == 0 {
            continue;
        }
        child.column = column;
        child.row = row;
        child.sizing_axes = sizing;
        child.flattened = true;
        child.extra_margin = extra;
        child.source_order = u16::MAX;
        // Общие базовые линии через подсетку — отдельный механизм (Blink
        // `GridBaselineAccumulator` по дереву размеров); здесь элемент
        // вкладывает только размеры.
        if child.align_self.keyword == AlignItemsKeyword::Baseline {
            child.align_self.keyword = AlignItemsKeyword::Start;
        }
        if child.justify_self.keyword == AlignItemsKeyword::Baseline {
            child.justify_self.keyword = AlignItemsKeyword::Start;
        }
        out.push(child);
    }
}
