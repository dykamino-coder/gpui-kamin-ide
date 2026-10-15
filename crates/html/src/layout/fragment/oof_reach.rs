//! Continuous containing-block coordinates for parallel positioned flows.
//!
//! Anchored extents belong to the containing block; static extents travel
//! through intervening flow boxes. Neither advances the in-flow cursor.

use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::OOF_OWN;
use crate::paint::stacking::fixed_cb_layer_box;
use crate::style::computed::Position;
use std::cell::Cell;

#[derive(Clone, Copy, Default)]
struct Extent {
    anchored: Option<f32>,
    flow: Option<f32>,
}

impl Extent {
    fn merge(&mut self, other: Self, start: f32) {
        let max = |a: Option<f32>, b: Option<f32>| match (a, b) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        self.anchored = max(self.anchored, other.anchored);
        self.flow = max(self.flow, other.flow.map(|v| start + v));
    }

    fn end(self) -> f32 {
        self.anchored.unwrap_or(0.0).max(self.flow.unwrap_or(0.0))
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Reach {
    pub(crate) legacy: f32,
    absolute: Extent,
    fixed: Extent,
    overflow: Option<f32>,
}

thread_local! {
    static PENDING: Cell<Option<(u64, Reach)>> = const { Cell::new(None) };
}

impl Reach {
    pub(crate) fn clear() {
        PENDING.set(None);
    }

    fn take(id: u64) -> Self {
        PENDING
            .take()
            .filter(|&(key, _)| key == id)
            .map_or(Self::default(), |(_, reach)| reach)
    }

    pub(crate) fn positioned(k: &Element, full: f32, top: Option<f32>, cx: ShapeCx) -> Self {
        if cx.paged || cx.legacy_oof {
            return Self {
                legacy: if k.style.position == Some(Position::Absolute) {
                    (top.unwrap_or(0.0) + full).max(0.0)
                } else {
                    0.0
                },
                ..Self::default()
            };
        }
        let mut reach = Self::take(k.node_id);
        let extent = if let Some(top) = top {
            Extent {
                anchored: Some(top + full),
                flow: None,
            }
        } else {
            Extent {
                anchored: None,
                flow: Some(full),
            }
        };
        match k.style.position {
            Some(Position::Absolute) => reach.absolute.merge(extent, 0.0),
            Some(Position::Fixed) => reach.fixed.merge(extent, 0.0),
            _ => {}
        }
        reach
    }

    pub(crate) fn child(k: &Element, full: f32, cx: ShapeCx) -> (f32, Self) {
        if cx.paged || cx.legacy_oof || !visible_overflow(&k.style) {
            return (full, Self::default());
        }
        let mut reach = Self::take(k.node_id);
        let own = OOF_OWN
            .with(|m| m.borrow().get(&k.node_id).copied())
            .filter(|&(_, measured)| (measured - full).abs() < 0.01)
            .map_or(full, |(own, _)| own);
        if full > own + 0.01 {
            reach.overflow = Some(full);
        }
        (own, reach)
    }

    pub(crate) fn merge(&mut self, other: Self, start: f32) {
        self.legacy = self.legacy.max(start + other.legacy);
        self.absolute.merge(other.absolute, start);
        self.fixed.merge(other.fixed, start);
        if let Some(end) = other.overflow {
            self.overflow = Some(self.overflow.unwrap_or(0.0).max(start + end));
        }
    }

    pub(crate) fn finish(mut self, c: &Element, cx: ShapeCx) -> f32 {
        if cx.paged || cx.legacy_oof {
            return if crate::text::inline::establishes_cb(&c.style) {
                self.legacy
            } else {
                0.0
            };
        }
        let mut end = self.overflow.take().unwrap_or(0.0);
        if crate::text::inline::establishes_cb(&c.style) {
            end = end.max(self.absolute.end());
            self.absolute = Extent::default();
        }
        if fixed_cb_layer_box(&c.style) {
            end = end.max(self.fixed.end());
            self.fixed = Extent::default();
        }
        PENDING.set(Some((c.node_id, self)));
        end
    }
}

#[cfg(test)]
mod tests;
