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
use crate::style::values::value::Len;
use std::collections::HashMap;
mod gate;
pub(crate) use gate::gate;
pub(crate) mod solve;
pub(crate) mod watch;

/// Attribute marking a stack child that takes this path (set by the measure
/// pass of the multicol container, read by the stack construction).
pub(crate) const MARK: &str = "kamin-paginate";

/// A break opportunity whose position is decided on the laid-out copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cand {
    /// Monolithic box (css-break-4 §4.1); `soft` — only `break-inside: avoid`,
    /// which may still break when it does not fit a whole fragmentainer.
    Mono { soft: bool },
    /// Forced break before the box (css-break-4 §3.1); `mt` — its margin,
    /// preserved after a forced break (§5.2).
    Forced { mt: f32 },
}

fn px_or_zero(l: &Option<Len>) -> Option<f32> {
    match l {
        None | Some(Len::Auto) => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    }
}

fn row_flex(s: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::FlexDir;
    matches!(s.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(
            s.flex_dir,
            None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
        )
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

fn walk(p: &Element, depth: u8, out: &mut Vec<(u64, Cand)>) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let mut force_next = false;
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
        let forced = force_next || k.style.break_before_force;
        force_next = k.style.break_after_force;
        // A forced break before an item of a flex row breaks the whole row
        // (Blink `row_break_between`), not just the item; clipped overflow is
        // not part of the flow thread.
        if (forced && row_flex(&p.style))
            || (!crate::layout::page::paged::visible_overflow(&k.style) && !monolith_box(k))
        {
            return None;
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
                    mt: px_or_zero(&k.style.margin.top)?,
                },
            ));
        }
        walk(k, depth - 1, out)?;
    }
    Some(())
}

/// Apply pagination struts (`node_id → px`) to a copy of the DOM: a block
/// parent gets an empty spacer box before the pushed box; a flex or grid item
/// gets the strut added to its block-start margin, so the line or track grows
/// with it. Returns `false` when a strut cannot be expressed.
pub(crate) fn apply(c: &mut Element, struts: &HashMap<u64, f32>) -> bool {
    if struts.is_empty() {
        return true;
    }
    let as_margin = layout_parent(&c.style);
    let mut i = 0;
    while i < c.children.len() {
        let mut spacer = None;
        if let Node::Element(k) = &mut c.children[i] {
            if let Some(&s) = struts.get(&k.node_id).filter(|s| **s > 0.0) {
                if as_margin {
                    let Some(m) = px_or_zero(&k.style.margin.top) else {
                        return false;
                    };
                    k.style.margin.top = Some(Len::Px(m + s));
                } else {
                    spacer = Some(spacer_box(k.node_id, s));
                }
            }
            if !apply(k, struts) {
                return false;
            }
        }
        if let Some(sp) = spacer {
            c.children.insert(i, Node::Element(sp));
            i += 1;
        }
        i += 1;
    }
    true
}

fn spacer_box(id: u64, h: f32) -> Element {
    Element {
        list_item: None,
        node_id: id ^ 0x5041_4749_4E41_5445,
        anim: None,
        tag: "div".to_string(),
        style: crate::style::computed::Computed {
            display: Some(Display::Block),
            height: Some(Len::Px(h)),
            ..Default::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: Vec::new(),
        inline: false,
    }
}

/// Rebuild of a paginated child's copies with a strut map: `(main, extra
/// copies)` for `n` copies in all, or `None` when the struts cannot be
/// expressed.
pub(crate) type Rebuild =
    dyn Fn(&HashMap<u64, f32>, usize) -> Option<(gpui::AnyElement, Vec<gpui::AnyElement>)>;

/// A stack child paginated by its own layout (see the module docs).
pub struct Paginate {
    pub(crate) cands: Vec<(u64, Cand)>,
    pub(crate) rebuild: Box<Rebuild>,
}
