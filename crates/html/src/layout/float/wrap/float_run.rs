//! A float run starting at node `i`: the measured host when the run and its
//! tail are expressible in it; otherwise the lowerings for runs not yet
//! representable in the shared plan: a lone float as a flow box, a
//! shape-outside host, or a row of floats beside a growing column.

use super::lone::wrap_lone_float;
use super::prepare::{host_for_run, wrap_into_host};
use super::row::push_float_row;
use super::run::{gather_floaters, run_min_width, split_rest, strut_for_next};
use super::shaped::{shaped_run_of, wrap_shaped_run};
use crate::dom::{Element, Node};
use crate::layout::float::FloatCb;
use crate::layout::float::clear::{bfc_no_fit, clears_side, leading_clear};
use crate::style::computed::Computed;
use std::ops::ControlFlow;

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_float_run(
    parent: &Computed,
    cb_top_open: bool,
    em: f32,
    measured_ok: bool,
    parent_bfc: bool,
    cell_bfc: bool,
    cb: FloatCb,
    nodes: &mut Vec<Node>,
    out: &mut Vec<Node>,
    i: &mut usize,
    side: i8,
) -> ControlFlow<()> {
    let cb_width = parent.width;
    let cb_px = cb.margin_basis;
    let hosted = host_for_run(em, measured_ok, cb, &*nodes, &*out, *i);
    if let ControlFlow::Break(_) = wrap_into_host(
        parent,
        cb_top_open,
        parent_bfc,
        cell_bfc,
        &*nodes,
        out,
        i,
        hosted,
    ) {
        return ControlFlow::Break(());
    }
    let mut floaters: Vec<Element> = vec![];
    let mut sides: Vec<i8> = vec![];
    let mut j = *i;
    gather_floaters(parent, &*nodes, *i, side, &mut floaters, &mut sides, &mut j);
    let run_min_w = run_min_width(&*nodes, *i, j);
    let separates = |next: &Element| {
        clears_side(next.style.clear, side)
            || clears_side(leading_clear(next), side)
            || bfc_no_fit(next, run_min_w, cb_px)
    };
    let (mut rest, mut out_of_flow) = split_rest(&*nodes, &mut j, separates);
    strut_for_next(cb_top_open, nodes, &*out, j, separates, &mut rest);
    if let ControlFlow::Break(_) = wrap_lone_float(
        parent,
        out,
        i,
        side,
        &mut floaters,
        j,
        &mut rest,
        &mut out_of_flow,
    ) {
        return ControlFlow::Break(());
    }
    let shaped_run = shaped_run_of(&floaters);
    if shaped_run && sides.iter().any(|s| *s != side) {
        // A run of floats on BOTH sides: `shape_flow` reads each float's
        // side from the float itself (css-shapes-1 §1 — every float's
        // shape narrows its own side of the line boxes).
        for (f, s) in floaters.iter_mut().zip(sides.iter()) {
            f.style.float = Some(*s);
        }
    }
    if let ControlFlow::Break(_) = wrap_shaped_run(
        parent,
        cb_width,
        out,
        i,
        side,
        &mut floaters,
        j,
        &mut rest,
        &mut out_of_flow,
        shaped_run,
    ) {
        return ControlFlow::Break(());
    }
    push_float_row(out, floaters, sides, rest);
    out.extend(out_of_flow);
    *i = j;
    ControlFlow::Continue(())
}
