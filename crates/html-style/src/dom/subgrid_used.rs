//! Используемые списки дорожек подсетки — до размещения и разрешения имён линий.
//!
//! css-grid-2 §subgrid-listing: без родительской сетки или в независимом
//! контексте форматирования используемое значение — `none`, и ось подсетки
//! очищается целиком (дорожки, имена линий, `repeat(auto-*)`). Контейнер лунок
//! открывает подсетке только ось своей решётки, с учётом ортогонального
//! письма; `display: contents` прозрачен для контекста сетки.
use super::Node;
use crate::style::computed::{Computed, Display};

#[derive(Clone, Copy)]
struct GridContext {
    vertical: bool,
    lanes_rows: Option<bool>,
}

pub(super) fn adjust(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
) -> Result<(), crate::budget::admission::Error> {
    walk(admission, nodes, None, false)
}

fn walk(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
    parent: Option<GridContext>,
    inherited_vertical: bool,
) -> Result<(), crate::budget::admission::Error> {
    crate::dom::traversal::preorder(
        admission,
        nodes,
        (parent, inherited_vertical),
        |element, &(parent, inherited_vertical)| {
            let style = element.style.cascaded_mut();
            let vertical = style.vertical_decl().unwrap_or(inherited_vertical);
            let linked_rows = parent.and_then(|grid| {
                grid.lanes_rows.map(|rows| {
                    if grid.vertical == vertical {
                        rows
                    } else {
                        !rows
                    }
                })
            });
            let grid = matches!(
                style.reset_box_layout.display,
                Some(Display::Grid | Display::InlineGrid)
            );
            let independent = !grid || super::fixup_grid::subgrid_inhibited(style);
            if style.reset_grid.subgrid_cols
                && (parent.is_none() || independent || linked_rows == Some(true))
            {
                clear_columns(style);
            }
            if style.reset_grid.subgrid_rows
                && (parent.is_none() || independent || linked_rows == Some(false))
            {
                clear_rows(style);
            }
            style.reset_grid.subgrid =
                style.reset_grid.subgrid_cols || style.reset_grid.subgrid_rows;
            let next = if style.reset_box_layout.display == Some(Display::Contents) {
                parent
            } else if grid {
                Some(GridContext {
                    vertical,
                    lanes_rows: style
                        .reset_grid
                        .lanes_taffy
                        .then(|| super::fixup_grid::lanes_row_dir(style)),
                })
            } else {
                None
            };
            Some((next, vertical))
        },
    )
}

fn clear_columns(style: &mut Computed) {
    style.reset_grid.subgrid_cols = false;
    style.reset_grid.grid_cols = None;
    style.reset_grid.grid_tracks = None;
    style.reset_grid.grid_col_line_names = None;
    style.reset_grid.auto_repeat_cols = None;
    style.reset_grid.auto_repeat_body_cols = None;
    style.reset_grid.grid_auto_fill_min = None;
    style.reset_grid.grid_auto_fill_tracks.clear();
}

fn clear_rows(style: &mut Computed) {
    style.reset_grid.subgrid_rows = false;
    style.reset_grid.grid_rows = None;
    style.reset_grid.grid_row_line_names = None;
    style.reset_grid.auto_repeat_rows = None;
    style.reset_grid.auto_repeat_body_rows = None;
    style.reset_grid.grid_auto_fill_row = None;
}
