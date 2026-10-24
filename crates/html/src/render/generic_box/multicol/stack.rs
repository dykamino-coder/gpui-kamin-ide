//! Сборка стопки колонок для блочных детей по мере `layout::multicol::measure`.

use crate::dom::Element;
use crate::dom::whitespace::is_blank;
use crate::layout::multicol::container::multicol_column_stack;
use crate::layout::multicol::measure::{StackPlan, stack_plan};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, Styled, px};

#[cfg(test)]
mod tests;

#[allow(clippy::too_many_arguments)]
pub(crate) fn stack_box(
    mut d: gpui::Div,
    e: &Element,
    merged: BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    cols: u16,
    column_width: Option<Len>,
    used_gap: f32,
    row_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    box_h: Option<f32>,
    col_inline_size: Option<Len>,
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
) -> Result<(gpui::Div, BoxStyle), AnyElement> {
    let StackPlan {
        line_col_w,
        layout_col_w,
        direct_oof,
        oof_static,
        nested_auto,
        nested_whole,
        measured_kids,
        stackable,
    } = stack_plan(
        e,
        &merged,
        inherited,
        opts,
        cols,
        used_gap,
        col_vert,
        col_rl,
        col_h,
        box_h,
        col_inline_size,
        rows,
        nest_rows,
    );
    if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
        return Err(multicol_column_stack(
            d,
            e,
            inherited,
            merged,
            opts,
            kids,
            cols,
            column_width,
            used_gap,
            row_gap,
            col_axis,
            col_vert,
            col_rl,
            col_h,
            (line_col_w, layout_col_w),
            rows,
            nest_rows,
            nest_phase,
            direct_oof,
            oof_static,
            nested_auto,
            nested_whole,
            measured_kids,
        ));
    }
    // An empty multicol has no column boxes to place. In particular, the
    // fallback grid must not give an empty shrink-to-fit box intrinsic gaps
    // (css-contain/contain-size-multicol-002 compares it with size containment).
    let count = e.children.iter().filter(|n| !is_blank(n)).count();
    if count == 0 {
        return Ok((d, merged));
    }
    let rows = count.div_ceil(cols as usize).max(1) as u16;
    let gap = used_gap;
    d = d
        .grid()
        .grid_template_cols((0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect())
        .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
        .gap_x(px(gap));
    d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
    Ok((d, merged))
}
