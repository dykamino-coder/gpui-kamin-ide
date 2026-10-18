//! Layout-driven pagination of one fragmentation-context child (first slice of
//! the break-token migration, `agents/cfrag/design.md`).
//!
//! The fragmentainer geometry (block size `H`, start offset `y0` of the child in
//! its first fragmentainer) becomes an input of the child's layout. Break
//! decisions are taken on the LAID-OUT geometry of the child's copy instead of
//! the DOM estimate `shape_full`: a monolithic box that would straddle a
//! fragmentainer boundary, or a box with a forced break before it, gets a
//! pagination strut and the copy is laid out again. The growth this causes is
//! propagated by the ordinary layout algorithms (a flex row grows to its pushed
//! item, stretched siblings follow, later rows move), which is the row
//! expansion of css-flexbox-1 §12 / Blink `flex_layout_algorithm.cc:2560-2600`.
//! The resulting copy is a flow thread whose content is aligned to the
//! fragmentainer grid, so it is sliced at the column edges.

use crate::dom::{Element, Node};
use crate::layout::fragment::clone::monolith_box;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;
use crate::style::values::value::{Len, px_auto_zero};
mod gate;
pub(crate) use gate::gate;
mod apply;
pub(crate) use apply::{Strut, Struts, apply};
pub(crate) mod solve;
pub(crate) mod watch;

/// A break opportunity whose position is decided on the laid-out copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cand {
    /// Monolithic box (css-break-4 §4.1); `soft` — only `break-inside: avoid`,
    /// which may still break when it does not fit a whole fragmentainer.
    Mono { soft: bool },
    /// Forced break before the box (css-break-4 §3.1); `mt` — its margin,
    /// preserved after a forced break (§5.2).
    Forced { mt: f32 },
    /// Item of a column flex container: never breaks by itself; its main
    /// size is fixed before fragmentation (css-flexbox-1 §12, Blink
    /// `GiveItemsFinalPositionAndSizeForFragmentation`), so a strut among
    /// its siblings must not shrink or grow it.
    Item,
    /// Item of a wrapping flex row whose rows are separated by `row-gap`: a
    /// gap adjoining a break is truncated (Blink
    /// `UpdateOffsetAdjustmentForSuppressedRowGap`).
    Gap { gap: f32, parent: u64 },
    /// `break-before: avoid` on this box (or `break-after: avoid` on `prev`):
    /// a fragmentainer boundary right at this edge violates it, and the
    /// earlier breakpoint before `prev` has the better appeal (css-break-3
    /// §4.4 rule 3; Blink `EarlyBreak`, `kBreakAppealViolatingBreakAvoid`).
    Avoid { prev: u64 },
}

fn row_flex(s: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::FlexDir;
    matches!(s.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(
            s.flex_dir,
            None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
        )
}

/// The px `row-gap` of a wrapping flex row.
pub(crate) fn row_gap(s: &crate::style::computed::Computed) -> Option<f32> {
    match s.gap {
        Some((Some(Len::Px(g)), _))
            if g > 0.0
                && row_flex(s)
                && s.flex_wrap == Some(true)
                && s.flex_wrap_reverse != Some(true) =>
        {
            Some(g)
        }
        _ => None,
    }
}

fn column_flex(s: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::FlexDir;
    matches!(s.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
}

fn layout_parent(s: &crate::style::computed::Computed) -> bool {
    matches!(
        s.display,
        Some(Display::Flex) | Some(Display::Grid) | Some(Display::InlineFlex)
    ) || s.webkit_box == Some(true)
}

/// Collect the candidates of `c`'s subtree, or `None` when the subtree holds
/// content whose geometry the copy's taffy tree does not expose (lines,
/// floats, tables, nested fragmentation contexts).
pub(crate) fn candidates(c: &Element) -> Option<Vec<(u64, Cand)>> {
    let mut out = Vec::new();
    walk(c, 8, &mut out)?;
    Some(out)
}

/// A single-line flex row with a `break-inside: avoid` item: breaking inside
/// the item breaks inside the row, and the earlier breakpoint before the row
/// has the better appeal (css-break-3 §4.4 rule 3; Blink break appeal
/// `kBreakAppealViolatingBreakAvoid` vs the row's `EarlyBreak`), so the row
/// is avoided as a whole.
pub(crate) fn avoid_row(p: &Element) -> bool {
    row_flex(&p.style)
        && p.style.flex_wrap != Some(true)
        && p.children.iter().any(|n| {
            matches!(n, Node::Element(k)
                if !out_of_flow(&k.style)
                    && k.style.break_inside_avoid
                    && !crate::layout::fragment::probe::size_monolith(k)
                    && crate::layout::page::paged::visible_overflow(&k.style))
        })
}

fn walk(p: &Element, depth: u8, out: &mut Vec<(u64, Cand)>) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let mut force_next = false;
    let mut prev: Option<(u64, bool)> = None;
    for n in p.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if out_of_flow(&k.style) {
            if k.style.float.is_some_and(|f| f != 0) {
                return None;
            }
            continue;
        }
        if k.inline
            || k.list_item.is_some()
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

/// Rebuild of a paginated child's copies with a strut map: `(main, extra
/// copies)` for `n` copies in all, or `None` when the struts cannot be
/// expressed.
pub(crate) type Rebuild =
    dyn Fn(&Struts, usize) -> Option<(gpui::AnyElement, Vec<gpui::AnyElement>)>;

/// A stack child paginated by its own layout (see the module docs).
pub struct Paginate {
    /// The child itself is an avoided row (`avoid_row`): it is placed as a
    /// `break-inside: avoid` box by the stack.
    pub(crate) avoid_root: bool,
    pub(crate) cands: Vec<(u64, Cand)>,
    pub(crate) rebuild: Box<Rebuild>,
    /// Recorder of the candidates' layout nodes in the rebuilt copies.
    pub(crate) watched: std::rc::Rc<watch::Watched>,
}
