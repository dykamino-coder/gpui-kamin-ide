//! Absolute motion of a spatial frame retained by a transform's source inputs.

use super::*;

/// The movement of the spatial frame where a painter captured its inputs.
/// Ancestor perspective and a nested scrolling child keep separate frames.
#[derive(Clone)]
pub struct ScrollMotion(Rc<dyn Fn() -> Point<Pixels>>);

impl std::fmt::Debug for ScrollMotion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollMotion").finish_non_exhaustive()
    }
}

impl PartialEq for ScrollMotion {
    fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) }
}

impl ScrollMotion {
    /// Absolute displacement since capture, in logical window pixels.
    pub fn delta(&self) -> Point<Pixels> { (self.0)() }
}

impl Window {
    /// Capture the current paint spatial frame without retaining the window.
    pub fn scroll_motion(&self) -> ScrollMotion {
        let scale = self.scale_factor();
        let mut at = self.next_frame.scroll_paint.open.last().map(|entry| entry.0);
        let mut chain = Vec::new();
        while let Some(index) = at {
            let node = &self.next_frame.scroll_nodes[index as usize];
            chain.push((node.source.clone(), node.offset));
            at = node.parent;
        }
        chain.reverse();
        ScrollMotion(Rc::new(move || chain.iter().fold(Point::default(), |parent, (source, old)| {
            source.replay_shift(parent, *old, source.offset(scale), scale)
        })))
    }
}
