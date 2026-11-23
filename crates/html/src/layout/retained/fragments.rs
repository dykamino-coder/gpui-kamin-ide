//! The line fragments of inline boxes between frames: where each inline box
//! lies, without a build (the geometry of `Element.getClientRects()`,
//! cssom-view-1 §6.1).
//!
//! A paragraph painted into the document's frame writes, for every line,
//! culled or not, the band owners with a run on it
//! (`gpui::TextRun::background_owner`) and the line's band into the slot of
//! its container box (`LayoutSlot::inline`), stamped with the frame's number
//! (`Store::epoch`). Blink keeps the fragment items of a block flow
//! (`FragmentItems`) and Gecko the continuation frames of an inline box
//! across frames; here a fragment is three numbers. The slot holds only for
//! the scene layer recorded in the frame of its stamp (`Store::fragments_hold`):
//! any frame that lays out again paints a new layer, so stale fragments
//! answer nothing rather than old places.
//!
//! A fragment spans the line's band in the block direction and the
//! paragraph's lines in the inline direction: an inline box reaches no
//! farther. A box nested in a band owner shares its owner's fragments, which
//! contain its own.

use crate::document::boxes::BoxTree;
use crate::dom::BoxId;
use crate::layout::reuse::Store;
use gpui::{Bounds, Pixels, ScaledPixels, point, px, size};
use std::cell::RefCell;
use std::rc::Rc;

/// One line of a band owner: the band's top and bottom, window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fragment {
    pub(crate) owner: u32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
}

/// The fragments of the paragraphs of one container box in one frame.
#[derive(Debug, Default)]
pub(crate) struct InlineFragments {
    /// `Store::epoch` of the frame that painted them.
    stamp: u64,
    /// Inline extent of the lines and the bleed of their ink beyond a band
    /// (`flat::cull::bleed`), window pixels; the device scale.
    left: f32,
    right: f32,
    bleed: f32,
    scale: f32,
    items: Vec<Fragment>,
}

impl InlineFragments {
    /// Start the fragments of a paragraph painted in frame `stamp`: a new
    /// frame replaces the last one's, another paragraph of the same frame
    /// adds to them (its lines widen the extent).
    pub(crate) fn begin(&mut self, stamp: u64, (left, right): (f32, f32), bleed: f32, scale: f32) {
        if self.stamp != stamp || self.items.is_empty() {
            self.items.clear();
            *self = Self {
                stamp,
                left,
                right,
                bleed,
                scale,
                items: std::mem::take(&mut self.items),
            };
            return;
        }
        self.left = self.left.min(left);
        self.right = self.right.max(right);
        self.bleed = self.bleed.max(bleed);
        self.scale = self.scale.max(scale);
    }

    /// Owner `owner` has a run on the line of band `top..bottom`.
    pub(crate) fn push(&mut self, owner: u32, top: f32, bottom: f32) {
        let f = Fragment { owner, top, bottom };
        if owner != 0 && self.items.last() != Some(&f) {
            self.items.push(f);
        }
    }

    pub(crate) fn stamp(&self) -> u64 {
        self.stamp
    }

    /// Whether `owner` has a fragment.
    pub(crate) fn has(&self, owner: u32) -> bool {
        self.items.iter().any(|f| f.owner == owner)
    }

    /// The line boxes of `owner`'s fragments in line order, window pixels.
    pub(crate) fn rects(&self, owner: u32) -> impl Iterator<Item = Bounds<Pixels>> + '_ {
        self.items
            .iter()
            .filter(move |f| f.owner == owner)
            .map(|f| Bounds {
                origin: point(px(self.left), px(f.top)),
                size: size(px(self.right - self.left), px(f.bottom - f.top)),
            })
    }

    /// The device-pixel area `owner`'s ink can reach, its fragments grown by
    /// the bleed and `outset`; `None` without a fragment.
    pub(crate) fn ink(&self, owner: u32, outset: f32) -> Option<Bounds<ScaledPixels>> {
        let grow = self.bleed + outset;
        self.rects(owner)
            .map(|r| Bounds {
                origin: point(r.origin.x - px(grow), r.origin.y - px(grow)),
                size: size(r.size.width + px(2. * grow), r.size.height + px(2. * grow)),
            })
            .reduce(|a, b| a.union(&b))
            .map(|b| b.scale(self.scale))
    }
}

/// Where a paragraph writes its fragments: the slot of its container box.
pub(crate) struct FragmentTarget {
    pub(crate) boxes: Rc<RefCell<BoxTree>>,
    pub(crate) owner: BoxId,
    pub(crate) store: Rc<Store>,
}

impl FragmentTarget {
    /// Write the fragments `fill` pushes for a paragraph painted now, with
    /// the extent and bleed of its lines. A box tree borrowed elsewhere
    /// takes none: the slot keeps an older stamp, which holds for no layer
    /// of this frame.
    pub(crate) fn write(
        &self,
        extent: (f32, f32),
        bleed: f32,
        scale: f32,
        fill: impl FnOnce(&mut InlineFragments),
    ) {
        let Ok(mut boxes) = self.boxes.try_borrow_mut() else {
            self.store
                .fragment_refused(self.owner, "box tree borrowed during fragment paint");
            return;
        };
        let Some(slot) = boxes.slot_mut(self.owner) else {
            self.store
                .fragment_refused(self.owner, "fragment owner box missing during paint");
            return;
        };
        let fragments = slot.inline.get_or_insert_with(Default::default);
        fragments.begin(self.store.frame_epoch(), extent, bleed, scale);
        fill(fragments);
        self.store
            .fragment_refused(self.owner, "fragments published");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(stamp: u64) -> InlineFragments {
        let mut f = InlineFragments::default();
        f.begin(stamp, (10., 110.), 2., 1.);
        f.push(7, 0., 20.);
        f.push(7, 20., 40.);
        f.push(8, 20., 40.);
        f
    }

    #[test]
    fn a_box_over_two_lines_has_two_line_rects() {
        let f = painted(1);
        let rects: Vec<_> = f.rects(7).collect();
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[1].origin, point(px(10.), px(20.)));
        assert_eq!(rects[1].size, size(px(100.), px(20.)));
        assert!(!f.has(9));
    }

    #[test]
    fn the_ink_grows_by_the_bleed_and_the_outset() {
        let ink = painted(1).ink(8, 3.).unwrap();
        assert_eq!(ink.origin, point(ScaledPixels(5.), ScaledPixels(15.)));
        assert_eq!(ink.size, size(ScaledPixels(110.), ScaledPixels(30.)));
        assert!(painted(1).ink(9, 0.).is_none());
    }

    #[test]
    fn a_new_frame_replaces_and_the_same_frame_adds() {
        let mut f = painted(1);
        f.begin(1, (0., 50.), 1., 1.);
        f.push(9, 40., 60.);
        assert!(f.has(7) && f.has(9));
        assert_eq!(f.rects(9).next().unwrap().origin.x, px(0.));
        f.begin(2, (0., 50.), 1., 1.);
        f.push(9, 0., 20.);
        assert!(!f.has(7));
        assert_eq!(f.stamp(), 2);
    }
}
