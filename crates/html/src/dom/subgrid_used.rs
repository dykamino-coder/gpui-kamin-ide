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

pub(super) fn adjust(nodes: &mut [Node]) {
    walk(nodes, None, false);
}

fn walk(nodes: &mut [Node], parent: Option<GridContext>, inherited_vertical: bool) {
    for node in nodes {
        let Node::Element(element) = node else {
            continue;
        };
        let style = &mut element.style;
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
        let grid = matches!(style.display, Some(Display::Grid | Display::InlineGrid));
        let independent = !grid || super::fixup_grid::subgrid_inhibited(style);
        if style.subgrid_cols && (parent.is_none() || independent || linked_rows == Some(true)) {
            clear_columns(style);
        }
        if style.subgrid_rows && (parent.is_none() || independent || linked_rows == Some(false)) {
            clear_rows(style);
        }
        style.subgrid = style.subgrid_cols || style.subgrid_rows;
        let next = if style.display == Some(Display::Contents) {
            parent
        } else if grid {
            Some(GridContext {
                vertical,
                lanes_rows: style
                    .lanes_taffy
                    .then(|| super::fixup_grid::lanes_row_dir(style)),
            })
        } else {
            None
        };
        walk(&mut element.children, next, vertical);
    }
}

fn clear_columns(style: &mut Computed) {
    style.subgrid_cols = false;
    style.grid_cols = None;
    style.grid_tracks = None;
    style.grid_col_line_names = None;
    style.auto_repeat_cols = None;
    style.auto_repeat_body_cols = None;
    style.grid_auto_fill_min = None;
    style.grid_auto_fill_tracks.clear();
}

fn clear_rows(style: &mut Computed) {
    style.subgrid_rows = false;
    style.grid_rows = None;
    style.grid_row_line_names = None;
    style.auto_repeat_rows = None;
    style.auto_repeat_body_rows = None;
    style.grid_auto_fill_row = None;
}
