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
}

pub(super) struct Recording {
    flows: Vec<Flow>,
    axis: StackAxis,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            flows: Vec::new(),
            axis: StackAxis::Horizontal,
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
        paged: bool,
    ) -> Self {
        let axis = StackAxis::of(parent.wm());
        let mut flows = Vec::new();
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
                )
            })
            .collect();
        pending.reverse();
        while let Some((e, owner, mut parent)) = pending.pop() {
            let is_positioned = positioned(e.style.used().position);
            if is_positioned {
                let blocks = BlockBox::collect(
                    e,
                    ShapeCx {
                        paged,
                        viewport: Some(opts.viewport),
                        ..ShapeCx::COLUMNS
                    },
                    opts.cx.vcx(),
                );
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
                });
                parent = Some(index);
            }
            pending.extend(e.children.iter().rev().filter_map(|n| match n {
                Node::Element(c) => Some((c, owner, parent)),
                _ => None,
            }));
        }
        let mut recorder = Self::default();
        recorder.flows = Recording { flows, axis };
        recorder
    }

    pub(super) fn fragment_snapshot(&self, snapshot: &mut Snapshot, window: &mut Window) {
        let axis = self.flows.axis;
        let mut contexts = Vec::with_capacity(self.flows.flows.len());
        for flow in &self.flows.flows {
            let (parent_repeat, parent_fixed) = flow
                .parent
                .map_or((false, false), |parent| contexts[parent]);
            let repeated = parent_repeat || flow.policy.get() == FixedPolicy::RepeatPages;
            let fixed_content = parent_fixed || flow.fixed.get();
            contexts.push((repeated, fixed_content));
            let Some(&metrics) = snapshot.get(flow.id) else {
                continue;
            };
            let Some(owner_rect) = self.normal(flow.owner, window) else {
                continue;
            };
            let rect = metrics.normal_rect();
            let h = f32::from(if axis.is_vertical() {
                rect.size.width
            } else {
                rect.size.height
            });
            let geometry = flow.watch.flow.at(rect.origin, rect.size, axis, window);
            let blocks = flow.watch.blocks().expect("OOF owns a block recorder");
            let laid = flow
                .watch
                .laid_axis(rect.origin, rect.size, axis, window)
                .with_flow(&geometry);
            let over = crate::layout::fragment::flow::flow_end(blocks, &laid).unwrap_or(h);
            let breaks = flow
                .watch
                .breaks(rect.origin, rect.size, over, axis, window);
            snapshot.flows.push(Source {
                id: flow.id,
                owner: flow.owner,
                owner_rect,
                metrics,
                kid: Kid {
                    h,
                    over,
                    monolith: flow.monolith || blocks.inside_avoid,
                    cuts: breaks.cuts,
                    forced: breaks.forced,
                    solid: breaks.solid,
                    avoid: breaks.avoid,
                    keep: breaks.keep,
                    clone_dec: flow.clone_dec.then_some((blocks.head, blocks.tail)),
                    par: crate::layout::fragment::types::Par {
                        avoid_only: !flow.monolith && blocks.inside_avoid,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                inline: Geometry {
                    floats: geometry.float_contexts,
                    paragraphs: geometry.inline,
                    barriers: breaks.barriers,
                    keep: breaks.avoid_inside,
                },
                policy: if repeated {
                    FixedPolicy::RepeatPages
                } else {
                    FixedPolicy::Fragment
                },
                fixed: flow.fixed.get(),
                inside_fixed: parent_fixed,
                static_block: flow.static_block.get(),
            });
        }
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
