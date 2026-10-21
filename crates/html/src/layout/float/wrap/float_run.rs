//! Серия флоатов с узла `i` уходит в общий измеряемый хост (`band_flow`):
//! вместе с содержимым после неё, если этот хвост представим в плане, иначе
//! одна, а хвост за хостом идёт обычным потоком под флоатами.

use super::prepare::{host_for_run, wrap_into_host};
use crate::dom::{Element, Node};
use crate::layout::box_kind::is_blank;
use crate::layout::float::FloatCb;
use crate::layout::float::band_measured::{band_float_m, band_host_m};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn wrap_float_run(
    parent: &Computed,
    cb_top_open: bool,
    em: f32,
    measured_ok: bool,
    parent_bfc: bool,
    cell_bfc: bool,
    cb: FloatCb,
    nodes: &[Node],
    out: &mut Vec<Node>,
    i: &mut usize,
    vcx: crate::document::state::ValueCx<'_>,
) {
    // `floats_alone` normalizes every margin the plan cannot express, so a
    // run of floats always has a host; the guard only keeps the loop going.
    let Some(hosted) = host_for_run(em, measured_ok, cb, nodes, out, *i, vcx)
        .or_else(|| floats_alone(em, cb, nodes, *i, vcx))
    else {
        out.push(nodes[*i].clone());
        *i += 1;
        return;
    };
    wrap_into_host(
        parent,
        cb_top_open,
        parent_bfc,
        cell_bfc,
        nodes,
        out,
        i,
        hosted,
    );
}

/// The run's floats alone in a measured host, when their tail is not
/// representable in the shared plan. A margin the plan cannot express
/// (a `calc()` mixing a percentage of an unknown containing block) is
/// resolved here; a percentage without a basis is cyclic and counts as zero
/// (css-sizing-3 §5.2.1).
fn floats_alone(
    em: f32,
    cb: FloatCb,
    nodes: &[Node],
    i: usize,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Element, usize, Option<usize>, Vec<Node>)> {
    let run: Vec<Node> = nodes[i..]
        .iter()
        .take_while(|n| {
            is_blank(n) || matches!(n, Node::Element(e) if e.style.float.is_some_and(|f| f != 0))
        })
        .map(|n| match n {
            Node::Element(f) if !is_blank(n) && band_float_m(f, em).is_none() => {
                let mut f = f.clone();
                let font = f.style.font_cx().family.to_owned();
                let font = crate::style::computed::FontCx::new(&font, em);
                let m = &mut f.style.margin;
                for side in [&mut m.top, &mut m.right, &mut m.bottom, &mut m.left] {
                    if let Some(l) =
                        side.filter(|l| !matches!(l, Len::Px(_) | Len::Pct(_) | Len::Auto))
                    {
                        *side = Some(Len::Px(l.to_px(cb.margin_basis, font, vcx).unwrap_or(0.0)));
                    }
                }
                Node::Element(f)
            }
            n => n.clone(),
        })
        .collect();
    band_host_m(&run, 0, em, &[], cb, vcx)
        .map(|(host, next, lifted)| (host, i + next, None, lifted))
}
