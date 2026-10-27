//! Исходные OOF-потоки: собственные строки и разрывы, без вклада в normal flow.

use super::{PositionedMetrics, Snapshot, record::Recorder};
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::{blocks::BlockBox, inline::Geometry};
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::token::resume::oof::FixedPolicy;
use crate::layout::fragment::types::{Kid, StackAxis};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Position};
use gpui::{Bounds, Pixels, Window};
use std::{cell::Cell, rc::Rc};
pub(crate) mod paint;
mod roots;
pub(crate) use roots::continuation_roots;
use roots::follows;
mod snapshot;

#[derive(Clone)]
pub(crate) struct Source {
    pub(crate) id: u64,
    pub(crate) owner: u64,
    pub(crate) owner_rect: Bounds<Pixels>,
    pub(crate) metrics: PositionedMetrics,
    pub(crate) kid: Kid,
    pub(crate) inline: Geometry,
    /// Повтор предка переносится на всё его содержимое; исходный CB хранится в metrics.
    pub(crate) policy: FixedPolicy,
    pub(crate) fixed: bool,
    pub(crate) inside_fixed: bool,
    pub(crate) static_block: bool,
    /// Outermost enclosing absolute/fixed flow (itself at the top): its own
    /// capture paints every nested continuation once (`paint::continuation`).
    pub(crate) root: u64,
    /// The native source paints it; otherwise its legacy copies do (a box laid
    /// out outside the source, or held, and every flow of its family).
    pub(crate) native: bool,
    /// Its own flat transform, resolved against each of its fragments.
    pub(crate) transform: Option<crate::style::computed::FlatTransform>,
    /// Its place is resolved by the legacy copy that replays it.
    pub(crate) follows: bool,
}

struct Flow {
    id: u64,
    parent: Option<usize>,
    owner: u64,
    watch: Rc<Watched>,
    monolith: bool,
    clone_dec: bool,
    policy: Cell<FixedPolicy>,
    fixed: Cell<bool>,
    static_block: Cell<bool>,
    /// Its place is resolved by its legacy copy (`roots::follows`).
    follows: bool,
    transform: Option<crate::style::computed::FlatTransform>,
}

pub(super) struct Recording {
    flows: Vec<Flow>,
    axis: StackAxis,
    pub(super) families: std::collections::HashMap<u64, u64>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            flows: Vec::new(),
            axis: StackAxis::Horizontal,
            families: Default::default(),
        }
    }
}

impl Recording {
    pub(super) fn policy(&self, id: u64, style: &BoxStyle) {
        if let Some(flow) = self.flows.iter().find(|f| f.id == id) {
            flow.fixed.set(style.position == Some(Position::Fixed));
            let edges = if self.axis.is_vertical() {
                [style.inset.left, style.inset.right]
            } else {
                [style.inset.top, style.inset.bottom]
            };
            flow.static_block.set(
                !edges
                    .into_iter()
                    .any(crate::layout::positioned::predicates::edge_set),
            );
            flow.policy.set(FixedPolicy::for_box(
                style.position == Some(Position::Fixed),
                style.fixed_cb_node != 0,
            ));
        }
    }
}

impl Recorder {
    pub(super) fn for_fragments<'a>(
        kids: impl Iterator<Item = &'a Element>,
        parent: &BoxStyle,
        opts: &RenderOpts,
    ) -> Self {
        let axis = StackAxis::of(parent.wm());
        let mut flows = Vec::new();
        let mut families = std::collections::HashMap::new();
        let mut pending: Vec<_> = kids
            .map(|e| {
                (
                    e,
                    if positioned(e.style.used().position) {
                        0
                    } else {
                        e.node_id
                    },
                    None,
                    false,
                )
            })
            .collect();
        pending.reverse();
        while let Some((e, owner, mut parent, nested)) = pending.pop() {
            // A nested multicol fragments its own absolute/fixed descendants: they
            // stay plain nodes of the enclosing family here (css-multicol-1 §2).
            let is_positioned = !nested && positioned(e.style.used().position);
            if is_positioned {
                let mut blocks = BlockBox::collect(
                    e,
                    // Named pages propagate through class A break points of the
                    // root's normal flow only: an absolute box's content never
                    // changes the page (css-page-3 §4.1, Blink).
                    ShapeCx {
                        paged: false,
                        viewport: Some(opts.viewport),
                        ..ShapeCx::COLUMNS
                    },
                    opts.cx.vcx(),
                );
                // css-break-3 §3.1/§3.2: break-before/after/inside do not apply
                // to absolutely positioned boxes; their content still breaks.
                blocks.own_breaks = (false, false, false, false);
                blocks.inside_avoid = false;
                let index = flows.len();
                flows.push(Flow {
                    id: e.node_id,
                    parent,
                    owner,
                    watch: Watched::for_blocks_axis(blocks, axis, true),
                    monolith: crate::layout::fragment::clone::content_monolith_blockified(e),
                    clone_dec: e.style.used().bdb_clone,
                    policy: Cell::new(FixedPolicy::Fragment),
                    fixed: Cell::new(false),
                    static_block: Cell::new(false),
                    follows: follows(e),
                    transform: crate::style::computed::FlatTransform::of(&e.style.used()),
                });
                parent = Some(index);
            }
            let inner =
                nested || crate::layout::multicol::spanner::multicol_container(&e.style.used());
            pending.extend(e.children.iter().rev().filter_map(|n| match n {
                Node::Element(c) => Some((c, owner, parent, inner)),
                _ => None,
            }));
            families.insert(e.node_id, parent.map_or(owner, |index| flows[index].id));
        }
        let mut recorder = Self::default();
        recorder.flows = Recording {
            flows,
            axis,
            families,
        };
        recorder
    }
}

// Float остаётся потоком своего владельца P06, а не abspos/fixed continuation.
fn positioned(position: Option<Position>) -> bool {
    matches!(position, Some(Position::Absolute | Position::Fixed))
}

/// Только указатель рекордера добавляется в дерево; тяжёлый RenderOpts лежит в heap.
pub(crate) fn options(e: &Element, opts: &RenderOpts) -> Option<Box<RenderOpts>> {
    let recorder = opts.cx.position.geometry_record.as_ref()?;
    let flow = recorder.flows.flows.iter().find(|f| f.id == e.node_id)?;
    let mut own = Box::new(opts.clone());
    own.frag.watch = Some(flow.watch.clone());
    Some(own)
}

#[cfg(test)]
mod tests;
