//! Адаптер LayoutId → исходная геометрия CB/OOF; prepaint и visual offset не читаются.

use super::{Dependencies, PositionedMetrics, Snapshot};
use crate::document::state::ValueCx;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::fragment::geometry::Node;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::SpotCell;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::px_lossy_zero;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, point, px, size};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
mod static_position;
use static_position::StaticPosition;

#[derive(Default)]
pub(crate) struct Recorder {
    nodes: RefCell<HashMap<u64, Entry>>,
    spots: RefCell<HashMap<u64, StaticPosition>>,
    pub(super) flows: super::fragmentation::Recording,
}

struct Entry {
    node: Node,
    cb: u64,
    border: [f32; 4],
    positioned: bool,
}

impl Entry {
    fn padding(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let [t, r, b, l] = self.border;
        Bounds {
            origin: bounds.origin + point(px(l), px(t)),
            size: size(
                (bounds.size.width - px(l + r)).max(px(0.)),
                (bounds.size.height - px(t + b)).max(px(0.)),
            ),
        }
    }
}

impl Recorder {
    pub(super) fn normal(&self, id: u64, window: &Window) -> Option<Bounds<Pixels>> {
        self.nodes.borrow().get(&id)?.node.bounds(window)
    }

    pub(crate) fn spot(
        &self,
        id: u64,
        spot: &SpotCell,
        line: Option<super::super::LineBoxSlot>,
        style: &Computed,
        vcx: ValueCx<'_>,
    ) {
        let now = spot.get();
        let rtl = now.rtl && !now.vertical && !now.rotated;
        self.spots.borrow_mut().insert(
            id,
            StaticPosition::Probe(spot.clone(), line, margin_offset(style, rtl, vcx)),
        );
    }

    pub(crate) fn inline_spot(
        &self,
        id: u64,
        line: super::super::LineBoxSlot,
        rtl: bool,
        style: &Computed,
        vcx: ValueCx<'_>,
    ) {
        let margin = margin_offset(style, rtl, vcx);
        self.spots
            .borrow_mut()
            .insert(id, StaticPosition::Inline { line, rtl, margin });
    }

    pub(crate) fn snapshot(&self, window: &Window) -> Snapshot {
        let nodes = self.nodes.borrow();
        let spots = self.spots.borrow();
        let mut snapshot = Snapshot::default();
        for (&id, entry) in nodes.iter().filter(|(_, e)| e.positioned) {
            let Some(cb) = nodes.get(&entry.cb) else {
                continue;
            };
            let (Some(layout), Some(cb_layout)) = (entry.node.read(window), cb.node.read(window))
            else {
                continue;
            };
            // Свободная ось без измеренного места остаётся незавершённой зависимостью.
            // Числовая оценка из соседей здесь недопустима.
            let (normal, static_rect, static_space) = match spots.get(&id) {
                Some(position) => {
                    let Some(resolved) = position.resolve(layout.normal, window) else {
                        continue;
                    };
                    resolved
                }
                None => (layout.normal, None, None),
            };
            let original_cb_rect = cb.padding(cb_layout.normal);
            snapshot
                .containing_blocks
                .insert(entry.cb, (cb.node, cb_layout.space));
            snapshot.positioned.insert(
                id,
                PositionedMetrics {
                    cb_id: entry.cb,
                    cb_fragment_id: cb.node,
                    original_cb_rect,
                    static_rect,
                    resolved_constraints: layout.space,
                    normal_offset: normal.origin - original_cb_rect.origin,
                    normal_size: normal.size,
                    dependencies: Dependencies {
                        containing_block: cb_layout.space,
                        positioned: layout.space,
                        static_position: static_space,
                    },
                },
            );
        }
        snapshot
    }
}

fn margin_offset(style: &Computed, rtl: bool, vcx: ValueCx<'_>) -> gpui::Point<Pixels> {
    use crate::layout::block::struts::margin_px;
    // CSS2 §10.3.7: static right задаёт правый margin edge, не левый border edge.
    let x = if rtl {
        style.margin.right
    } else {
        style.margin.left
    };
    point(
        px(margin_px(x, style, vcx).unwrap_or(0.) * if rtl { -1. } else { 1. }),
        px(margin_px(style.margin.top, style, vcx).unwrap_or(0.)),
    )
}

/// Декоратор исходного CSS-бокса сохраняет его LayoutId, не добавляя коробок solver.
pub(crate) fn wrap(id: u64, style: &BoxStyle, child: AnyElement, opts: &RenderOpts) -> AnyElement {
    match capture(id, style, opts) {
        Some(geometry) => geometry.wrap(child),
        None => child,
    }
}

/// Только метаданные записи переживают рекурсивную сборку детей, не весь BoxStyle.
pub(crate) fn capture(id: u64, style: &BoxStyle, opts: &RenderOpts) -> Option<Box<Recording>> {
    let recorder = opts.cx.position.geometry_record.as_ref()?;
    recorder.flows.policy(id, style);
    let b = style.borders();
    Some(Box::new(Recording {
        id,
        cb: if style.position == Some(crate::style::computed::Position::Fixed) {
            style.fixed_cb_node
        } else {
            style.cb_node
        },
        positioned: crate::layout::box_kind::out_of_flow(style),
        border: [b.top, b.right, b.bottom, b.left].map(px_lossy_zero),
        recorder: recorder.clone(),
    }))
}

pub(crate) struct Recording {
    id: u64,
    cb: u64,
    positioned: bool,
    border: [f32; 4],
    recorder: Rc<Recorder>,
}

impl Recording {
    pub(crate) fn wrap(self: Box<Self>, child: AnyElement) -> AnyElement {
        Record {
            geometry: self,
            child,
        }
        .hooked()
    }
}

struct Record {
    geometry: Box<Recording>,
    child: AnyElement,
}

impl Wrap for Record {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        let geometry = &self.geometry;
        geometry.recorder.nodes.borrow_mut().insert(
            geometry.id,
            Entry {
                node: Node::capture(id, window),
                cb: geometry.cb,
                positioned: geometry.positioned,
                border: geometry.border,
            },
        );
        id
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod static_tests;
