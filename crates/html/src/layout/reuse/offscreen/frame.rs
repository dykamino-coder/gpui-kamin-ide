//! The frame check of the leaves (`FlowReuse::validate`) and what it keeps.

use super::{Kept, Kind};
use crate::layout::reuse::FlowReuse;
use gpui::{Bounds, ScaledPixels, Window};
use std::collections::{HashMap, HashSet};

/// Exactness of a leaf against its block, in px.
const EXACT: f32 = 0.01;

impl FlowReuse {
    /// The leaves of this build before prepaint, `screen` the window mask
    /// grown by the interest margin. A leaf near the screen, a measured block
    /// whose margins or height its leaf did not repeat, or an exact leaf of
    /// another width is refused: `false`, and the rebuild of the frame builds
    /// those blocks. Every block's place and leaf are kept for later builds.
    pub(in crate::layout::reuse) fn validate_offscreen(
        &self,
        window: &mut Window,
        screen: Bounds<ScaledPixels>,
    ) -> bool {
        let scale = window.scale_factor();
        let scroll = self.offscreen.scroll.get().unwrap_or(0.0);
        let viewport = self.offscreen.viewport.get();
        let mut refused = HashSet::new();
        let mut kept = self.store.offscreen.kept.take();
        let slots = self.offscreen.slots.borrow();
        // Where the floats that reach past their blocks lie: a later leaf they
        // meet would not wrap its lines around them. A block's zone comes
        // after its own blocks (`built_block`), which its floats do not leave.
        let mut floats = Vec::new();
        for slot in slots.iter() {
            if matches!(slot.kind, Kind::Floats) {
                if let Some(id) = slot.layout.get() {
                    floats.push(window.layout_subtree_bounds(id));
                }
                continue;
            }
            let Some(id) = slot.layout.get() else {
                if slot.leaf() {
                    refused.insert(slot.node);
                }
                continue;
            };
            let bounds = window.layout_bounds(id);
            let (top, bottom) = (bounds.top().as_f32(), bounds.bottom().as_f32());
            // Under the root scroller the blocks lie at their unscrolled place.
            let shown = Bounds {
                origin: gpui::point(
                    ScaledPixels(bounds.origin.x.as_f32() * scale),
                    ScaledPixels((top - scroll) * scale),
                ),
                size: gpui::size(
                    ScaledPixels(bounds.size.width.as_f32() * scale),
                    ScaledPixels(bounds.size.height.as_f32() * scale),
                ),
            };
            let near = shown.intersects(&screen);
            let (snapshot, fits) = match slot.kind {
                Kind::Exact(s) => {
                    let wide = (bounds.size.width.as_f32() - s.width()).abs() <= EXACT;
                    (Some(s).filter(|_| wide), wide)
                }
                Kind::Measured => {
                    // Unrounded: a built block's box rounds at its place as
                    // the leaf's does.
                    let leaf = window.layout_snapshot(id);
                    let size = window.layout_size_unrounded(id);
                    let measured = slot.measured.get().filter(|m| {
                        leaf.is_some_and(|l| l.same_block_margins(m))
                            && (size.height.as_f32() - m.height()).abs() <= EXACT
                            && (size.width.as_f32() - m.width()).abs() <= EXACT
                    });
                    (measured, measured.is_some())
                }
                Kind::Built | Kind::Floats => (window.layout_snapshot(id), true),
            };
            let floated = floats.iter().any(|f| f.intersects(&bounds));
            let snapshot = snapshot.filter(|_| !slot.failed.get() && !floated);
            if slot.leaf() && (near || floated || !fits || slot.failed.get()) {
                refused.insert(slot.node);
            }
            kept.insert(
                slot.node,
                Kept {
                    snapshot,
                    at: slot.at,
                    span: (top, bottom),
                    viewport,
                },
            );
        }
        self.store.offscreen.kept.replace(kept);
        let ok = refused.is_empty();
        let mut all = self.store.offscreen.refused.take();
        all.extend(refused);
        self.store.offscreen.refused.set(all);
        ok
    }

    /// After the frame's paint: only the blocks this frame built or placed
    /// keep their leaves, so removed blocks leave nothing behind.
    pub(in crate::layout::reuse) fn finish_offscreen(&self) {
        self.store.offscreen.refused.take();
        let slots = self.offscreen.slots.borrow();
        let seen: HashSet<u64> = slots.iter().map(|s| s.node).collect();
        let leaves = || slots.iter().filter(|s| s.leaf());
        let mut stats = self.store.stats.get();
        stats.offscreen += leaves().count() as u64;
        self.store.stats.set(stats);
        // A paint-only edit of a box in a leaf shows no pixel of this frame.
        self.store.add_unpainted(leaves().map(|s| s.node));
        let mut kept = self.store.offscreen.kept.take();
        kept.retain(|node, _| seen.contains(node));
        if kept.capacity() > 2 * kept.len() + 16 {
            kept = kept.into_iter().collect::<HashMap<_, _>>();
        }
        self.store.offscreen.kept.replace(kept);
    }
}
