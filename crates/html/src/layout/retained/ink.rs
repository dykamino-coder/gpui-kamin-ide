//! The ink overflow of the paragraphs of a box that place no line
//! fragments, kept with the box between frames (`LayoutSlot::ink`).
//!
//! A paragraph a native paragraph does not paint (a row of pieces, a plain
//! styled text) writes no fragments (`fragments`): after the frame nothing
//! says where its inline boxes lie. Its paint is measured instead while it
//! paints: every primitive it offers, culled and deferred paint included and
//! composited groups and layers by their bounds (`gpui::InkSink`), and its
//! own box, which holds its lines. The measures of the box's paragraphs in
//! one frame are united and stamped with the frame's number, as fragments
//! are, and hold only for the scene layer of that frame
//! (`Store::fragments_hold`). Gecko keeps the ink overflow of every frame
//! with its border box (`nsIFrame::InkOverflowRect`), Blink the visual
//! overflow of every layout object; here only such paragraphs pay for it.
//!
//! A sink's taint (state read, listeners) unfits a measure for replay, not
//! for bounds: the primitives are offered all the same. A paragraph painted
//! outside the root environment has no box in window space: it keeps none.

use super::FragmentTarget;
use crate::hooks::{IntoHooked, Wrap};
use gpui::{AnyElement, App, Bounds, InkSinkList, Pixels, ScaledPixels, Window};
use std::rc::Rc;

/// `el`, a paragraph of the flow of `target`'s box that places no
/// fragments, measuring its ink into the box's slot when it paints.
pub(crate) fn measured(el: AnyElement, target: Option<FragmentTarget>) -> AnyElement {
    let Some(target) = target else {
        return el;
    };
    // A sink is opened while no paint measures into the list: at the build.
    let sinks = Rc::new(InkSinkList::default());
    let at = sinks.open();
    Measured {
        child: el,
        target,
        sinks,
        at,
    }
    .hooked()
}

struct Measured {
    child: AnyElement,
    target: FragmentTarget,
    sinks: Rc<InkSinkList>,
    at: u32,
}

impl Wrap for Measured {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let root = window.retained_root_environment();
        window.with_ink_sink((&self.sinks, self.at), |window| {
            self.child.paint(window, cx);
            // The paragraph's box holds its lines and their inline boxes but
            // for their padding and border, painted or not.
            window.offer_ink(bounds);
        });
        let ink = self.sinks.with(self.at, |sink| sink.bounds()).flatten();
        self.target.write_ink(ink.filter(|_| root));
    }
}

impl FragmentTarget {
    /// Unite `ink`, the ink overflow of a paragraph painted now, with the
    /// box's from this frame; `None` (unknown) voids the box's. A box tree
    /// borrowed elsewhere takes none: the slot keeps an older stamp.
    fn write_ink(&self, ink: Option<Bounds<ScaledPixels>>) {
        let (Some(boxes), Some(store)) = (self.boxes.upgrade(), self.store.upgrade()) else {
            return;
        };
        let Ok(mut boxes) = boxes.try_borrow_mut() else {
            return;
        };
        let Some(slot) = boxes.slot_mut(self.owner) else {
            return;
        };
        let stamp = store.frame_epoch();
        slot.ink = match (slot.ink, ink) {
            // An unknown paragraph of this frame keeps the box unknown.
            (Some((s, None)), _) if s == stamp => Some((stamp, None)),
            (Some((s, Some(kept))), Some(ink)) if s == stamp => {
                Some((stamp, Some(kept.union(&ink))))
            }
            (_, ink) => Some((stamp, ink)),
        };
    }
}
