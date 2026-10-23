//! Обход кандидатов для общего решателя измеренной фрагментации.

use super::{Cand, avoid_row, column_flex, row_flex, row_gap};
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::clone::{monolith_box, solid_box};
use crate::style::computed::Display;
use crate::style::values::value::px_auto_zero;

pub(super) fn walk(
    p: &Element,
    depth: u8,
    out: &mut Vec<(u64, Cand)>,
    vcx: ValueCx<'_>,
) -> Option<()> {
    // Native table rows use the table planner's geometry and break expansion;
    // they are not ordinary block boxes observed by the measured solver.
    if depth == 0 || crate::layout::fragment::table_bands::table_box(p) {
        return None;
    }
    let mut force_next = false;
    let mut prev: Option<(u64, bool)> = None;
    for n in p.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { continue };
        if out_of_flow(k.style.cascaded()) {
            if k.style.cascaded().float.is_some_and(|f| f != 0) {
                if solid_box(k) {
                    out.push((
                        k.node_id,
                        Cand::Float {
                            soft: k.style.cascaded().break_inside_avoid
                                && !crate::layout::fragment::probe::size_monolith(k)
                                && crate::layout::page::paged::visible_overflow(k.style.cascaded()),
                        },
                    ));
                } else {
                    walk(k, depth - 1, out, vcx)?;
                }
            }
            continue;
        }
        if k.inline {
            walk(k, depth - 1, out, vcx)?;
            continue;
        }
        if k.list_item.is_some()
            || crate::layout::multicol::spanner::multicol_container(k.style.cascaded())
            || !matches!(
                k.style.cascaded().display,
                None | Some(Display::Block) | Some(Display::Flex) | Some(Display::Grid)
            )
        {
            return None;
        }
        if column_flex(p.style.cascaded()) {
            out.push((k.node_id, Cand::Item));
        }
        if let Some(gap) = row_gap(p.style.cascaded()) {
            out.push((
                k.node_id,
                Cand::Gap {
                    gap,
                    parent: p.node_id,
                },
            ));
        }
        // Breaks propagate from the first/last child to the box edge
        // (css-break-3 §3.1 «break propagation»).
        if let Some((pid, avoid_after)) = prev
            && (avoid_after || k.style.cascaded().break_before_avoid)
        {
            out.push((pid, Cand::Item));
            out.push((k.node_id, Cand::Avoid { prev: pid }));
        }
        prev = Some((k.node_id, k.style.cascaded().break_after_avoid));
        let edge = crate::layout::fragment::breaks::edge_break;
        let forced = force_next || edge(k, false, vcx);
        force_next = edge(k, true, vcx);
        // A forced break before an item of a flex row breaks the whole row
        // (Blink `row_break_between`), not just the item; clipped overflow is
        // not part of the flow thread.
        if (forced && row_flex(p.style.cascaded()))
            || (!crate::layout::page::paged::visible_overflow(k.style.cascaded())
                && !monolith_box(k))
        {
            return None;
        }
        if avoid_row(k) {
            out.push((k.node_id, Cand::Mono { soft: true }));
            continue;
        }
        if monolith_box(k) {
            out.push((
                k.node_id,
                Cand::Mono {
                    soft: k.style.cascaded().break_inside_avoid
                        && !crate::layout::fragment::probe::size_monolith(k)
                        && crate::layout::page::paged::visible_overflow(k.style.cascaded()),
                },
            ));
            continue;
        }
        if forced {
            out.push((
                k.node_id,
                Cand::Forced {
                    mt: px_auto_zero(k.style.cascaded().margin.top)?,
                },
            ));
        }
        walk(k, depth - 1, out, vcx)?;
    }
    Some(())
}
