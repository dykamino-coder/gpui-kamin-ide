//! Candidate traversal for the shared measured fragmentation solver.

use super::{Cand, avoid_row, column_flex, row_flex, row_gap};
use crate::dom::{Element, Node};
use crate::layout::fragment::clone::monolith_box;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::px_auto_zero;

pub(super) fn walk(p: &Element, depth: u8, out: &mut Vec<(u64, Cand)>) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let mut force_next = false;
    let mut prev: Option<(u64, bool)> = None;
    for n in p.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { continue };
        if out_of_flow(&k.style) {
            if k.style.float.is_some_and(|f| f != 0) {
                if monolith_box(k) {
                    out.push((
                        k.node_id,
                        Cand::Float {
                            soft: k.style.break_inside_avoid
                                && !crate::layout::fragment::probe::size_monolith(k)
                                && crate::layout::page::paged::visible_overflow(&k.style),
                        },
                    ));
                } else {
                    walk(k, depth - 1, out)?;
                }
            }
            continue;
        }
        if k.inline {
            walk(k, depth - 1, out)?;
            continue;
        }
        if k.list_item.is_some()
            || crate::layout::multicol::spanner::multicol_container(&k.style)
            || !matches!(
                k.style.display,
                None | Some(Display::Block) | Some(Display::Flex) | Some(Display::Grid)
            )
        {
            return None;
        }
        if column_flex(&p.style) {
            out.push((k.node_id, Cand::Item));
        }
        if let Some(gap) = row_gap(&p.style) {
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
            && (avoid_after || k.style.break_before_avoid)
        {
            out.push((pid, Cand::Item));
            out.push((k.node_id, Cand::Avoid { prev: pid }));
        }
        prev = Some((k.node_id, k.style.break_after_avoid));
        let edge = crate::layout::fragment::breaks::edge_break;
        let forced = force_next || edge(k, false);
        force_next = edge(k, true);
        // A forced break before an item of a flex row breaks the whole row
        // (Blink `row_break_between`), not just the item; clipped overflow is
        // not part of the flow thread.
        if (forced && row_flex(&p.style))
            || (!crate::layout::page::paged::visible_overflow(&k.style) && !monolith_box(k))
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
                    soft: k.style.break_inside_avoid
                        && !crate::layout::fragment::probe::size_monolith(k)
                        && crate::layout::page::paged::visible_overflow(&k.style),
                },
            ));
            continue;
        }
        if forced {
            out.push((
                k.node_id,
                Cand::Forced {
                    mt: px_auto_zero(k.style.margin.top)?,
                },
            ));
        }
        walk(k, depth - 1, out)?;
    }
    Some(())
}
