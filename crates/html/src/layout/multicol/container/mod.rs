//! Многоколоночная ветвь `element()`: из готовых мер детей `(Element, Shape)` собирает
//! `ColumnStack` контейнера. Здесь число копий, высота заливки при `column-fill: auto`,
//! план `box-decoration-break: clone`, линейка или спецификация линеек промежутков для
//! `GapRulePainter`, щупы статической позиции внепоточных детей и их заместители после
//! стопки, слой содержащего блока `position: fixed` и флоаты соседями стопки.

use crate::dom::Element;
use crate::layout::build_contracts::Layer;
use crate::layout::fragment::Shape;
use crate::layout::multicol::gap_rules::multicol_gap_rule_spec;
use crate::layout::options::RenderOpts;
use crate::style::computed::stacking::fixed_cb_layer_box;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, ParentElement};
pub(super) mod kids;
use kids::{column_kids, stack_children};
mod clone_plan;
use clone_plan::clone_plan_of;
mod oof;
use oof::{column_stack_div, finish_column_stack, place_oof_statics};
mod spanner;
pub(crate) use spanner::multicol_spanner_segments;
mod fixed_height;
use fixed_height::fixed_column_height;

#[allow(clippy::too_many_arguments)]
pub(crate) fn multicol_column_stack(
    d: gpui::Div,
    e: &Element,
    inherited: &Computed,
    merged: BoxStyle,
    opts: &RenderOpts,
    kids: Vec<(Element, Shape)>,
    cols: u16,
    column_width: Option<Len>,
    used_gap: f32,
    row_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    col_h: Option<f32>,
    (line_col_w, layout_col_w): (Option<f32>, Option<f32>),
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
    direct_oof: Vec<Element>,
    oof_static: Vec<(usize, Element)>,
    nested_auto: std::cell::RefCell<Vec<u64>>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
) -> AnyElement {
    let fixed = fixed_column_height(
        &merged,
        &crate::layout::block::sizing::BoxDimensions::for_element(e, &merged),
        col_vert,
        col_h,
        nest_rows,
        &opts.cx.constraints,
    );
    let source = if !col_vert && rows.is_none() {
        kids.iter().map(|(c, _)| c.clone()).collect()
    } else {
        Vec::new()
    };
    let (e, inherited, opts) = (e.clone(), inherited.clone(), opts.clone());
    crate::layout::positioned::containing_block::metrics::layout::defer(
        source,
        merged.clone(),
        line_col_w.or(layout_col_w),
        opts,
        move |opts| {
            let (e, inherited, opts) = (&e, &inherited, &opts);
            let vcx = opts.cx.vcx();
            let copies = match rows {
                Some(r) => {
                    let per = r.h.unwrap_or(f32::MAX).max(1.0);
                    let span = kids
                        .iter()
                        .filter(|(c, _)| c.style.used().column_span != Some(true))
                        .map(|(_, s)| (s.height / per).ceil() as usize)
                        .max()
                        .unwrap_or(0);
                    span.saturating_add(cols as usize).saturating_add(1)
                }
                None => cols.max(1) as usize,
            };
            let deferred = (line_col_w.is_none() && nest_rows.is_none())
                .then(|| {
                    super::deferred::Recipe::new(
                        e, &merged, opts, cols, used_gap, fixed, col_axis, rows,
                    )
                })
                .flatten();
            let (kid_parent, kid_starts, kid_par, kids) = if deferred.is_some() {
                (Vec::new(), Vec::new(), Vec::new(), Vec::new())
            } else {
                crate::layout::fragment::with_lines(&merged, line_col_w, opts, |opts| {
                    column_kids(
                        &merged,
                        kids,
                        cols,
                        used_gap,
                        col_vert,
                        rows,
                        copies,
                        fixed,
                        (line_col_w, layout_col_w),
                        opts,
                    )
                })
            };
            let prepared: Vec<_> = kids
                .iter()
                .enumerate()
                .map(|(ix, (c, s))| {
                    crate::layout::multicol::stack_child::prepare(
                        ix,
                        c,
                        s,
                        kid_parent[ix].as_ref().unwrap_or(&merged),
                        opts,
                        &kid_par,
                        col_vert,
                        col_rl,
                        line_col_w,
                        fixed,
                        &measured_kids,
                    )
                })
                .collect();
            let clone_plan = clone_plan_of(
                cols, col_vert, rows, copies, fixed, &kid_par, &kids, &prepared, vcx, opts,
            );
            // All column rules use the canonical gap painter, including vertical writing modes.
            let gap_spec = multicol_gap_rule_spec(e, &merged, opts, used_gap, row_gap);
            let gap_items = gap_spec
                .as_ref()
                .map(|_| crate::layout::multicol::gap_rules::GapItems::default());
            let fixed_layer = fixed_cb_layer_box(&merged).then(|| {
                crate::layout::positioned::layer_scope::LayerScope::fixed_only(
                    &opts.cx.position.state,
                )
            });
            let children = if deferred.is_some() {
                Vec::new()
            } else {
                stack_children(
                    e,
                    &merged,
                    opts,
                    cols,
                    col_vert,
                    col_rl,
                    (line_col_w, layout_col_w),
                    rows,
                    nested_auto,
                    nested_whole,
                    measured_kids,
                    copies,
                    fixed,
                    kid_parent,
                    kid_par,
                    kids,
                    clone_plan,
                    prepared,
                )
            };
            let (children, oof_spots) =
                place_oof_statics(&merged, opts, &oof_static, kid_starts, children);
            let painter = gap_items.clone().zip(gap_spec);
            let d = d.children(painter.map(|(b, s)| {
                opts.cx
                    .contracts
                    .build
                    .layer(Layer::gap_rules(b, s, &opts.cx))
            }));
            let mut d = column_stack_div(
                d,
                e,
                inherited,
                &merged,
                cols,
                column_width,
                used_gap,
                col_axis,
                col_vert,
                col_rl,
                rows,
                nest_rows,
                nest_phase,
                fixed,
                &gap_items,
                children,
                opts,
                deferred,
            );
            if let Some(layer) = fixed_layer {
                d = d.children(layer.finish());
            }
            for oof in &direct_oof {
                d = d.child(opts.cx.contracts.build.element(oof, &merged, opts));
            }
            finish_column_stack(e, merged, opts, col_vert, oof_static, oof_spots, d)
        },
    )
}
