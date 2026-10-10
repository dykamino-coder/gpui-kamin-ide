//! Сросшиеся рамки колонок и групп колонок (border-collapse).

use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::ParentElement;

#[allow(clippy::too_many_arguments)]
pub(crate) fn collapsed_col_edges(
    cell_cols: &std::ops::Range<usize>,
    col_els: &[Option<&crate::dom::Element>],
    row_ix: i16,
    row_elements: &[&crate::dom::Element],
    inherited: &Computed,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    px_of: &impl Fn(Option<Len>) -> f32,
    mut d: gpui::Div,
) -> gpui::Div {
    for i in cell_cols.clone() {
        let Some(el) = col_els.get(i).copied().flatten() else {
            continue;
        };
        let b = el.style.borders();
        let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
        let hidden = el.style.border_side_styles.contains(&Some(1));
        if !(cw.iter().any(|w| *w > 0.0) || hidden) {
            continue;
        }
        let same = |j: i64| -> bool {
            j >= 0
                && col_els
                    .get(j as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|o| o.node_id == el.node_id)
        };
        let left_edge = !same(i as i64 - 1);
        let right_edge = !same(i as i64 + 1);
        let last_row = row_ix as usize >= row_elements.len();
        let widths = [
            if row_ix == 1 { cw[0] } else { 0.0 },
            if right_edge { cw[1] } else { 0.0 },
            if last_row { cw[2] } else { 0.0 },
            if left_edge { cw[3] } else { 0.0 },
        ];
        if !(widths.iter().any(|w| *w > 0.0) || hidden) {
            continue;
        }
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |k: usize| {
            el.style.border_colors[k]
                .or(el.style.border_color)
                .or(el.style.color)
                .or(inherited.color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style = |k: usize| {
            el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
        };
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            widths,
            colors,
            styles,
            2,
            el.node_id as u32,
            [0.0; 4],
        ));
    }
    d
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn collapsed_colgroup_edges(
    cell_cols: std::ops::Range<usize>,
    grp_els: &[Option<&Element>],
    row_ix: i16,
    row_elements: &[&Element],
    inherited: &Computed,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    px_of: &impl Fn(Option<Len>) -> f32,
    mut d: gpui::Div,
) -> gpui::Div {
    for i in cell_cols {
        let Some(el) = grp_els.get(i).copied().flatten() else {
            continue;
        };
        let b = el.style.borders();
        let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
        let hidden = el.style.border_side_styles.contains(&Some(1));
        if !(cw.iter().any(|w| *w > 0.0) || hidden) {
            continue;
        }
        let same = |j: i64| -> bool {
            j >= 0
                && grp_els
                    .get(j as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|o| o.node_id == el.node_id)
        };
        let left_edge = !same(i as i64 - 1);
        let right_edge = !same(i as i64 + 1);
        let last_row = row_ix as usize >= row_elements.len();
        let widths = [
            if row_ix == 1 { cw[0] } else { 0.0 },
            if right_edge { cw[1] } else { 0.0 },
            if last_row { cw[2] } else { 0.0 },
            if left_edge { cw[3] } else { 0.0 },
        ];
        if !(widths.iter().any(|w| *w > 0.0) || hidden) {
            continue;
        }
        let black = crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |k: usize| {
            el.style.border_colors[k]
                .or(el.style.border_color)
                .or(el.style.color)
                .or(inherited.color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
        let side_style = |k: usize| {
            el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
        };
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        d = d.child(crate::layout::table::paint::edge_probe(
            table_edges.clone(),
            widths,
            colors,
            styles,
            1,
            el.node_id as u32,
            [0.0; 4],
        ));
    }
    d
}
