//! Painting isolation and deferred drawing impose different geometry boundaries.

use std::cell::Cell;

#[derive(Clone, Copy, Default)]
pub(crate) struct Depth {
    paint: usize,
    deferred: usize,
}

thread_local! {
    static DEPTH: Cell<Depth> = const { Cell::new(Depth { paint: 0, deferred: 0 }) };
}

pub(crate) fn snapshot() -> Depth {
    DEPTH.with(Cell::get)
}

pub(crate) fn inside() -> bool {
    snapshot().paint > 0
}

pub(crate) fn deferred() -> bool {
    snapshot().deferred > 0
}

pub(crate) struct Guard {
    paint: bool,
    deferred: bool,
}

impl Guard {
    pub(crate) fn enter(deferred: bool, stacking: bool) -> Self {
        // CSS2 §10.1 and CSS Containment 2 §3.2: a positioned descendant
        // still uses its containing ancestor across intervening static boxes.
        // A stacking context only limits paint ordering (CSS2 Appendix E);
        // actual GPUI deferred drawing must additionally limit relocation.
        let guard = Self {
            paint: deferred || stacking,
            deferred,
        };
        DEPTH.with(|depth| {
            let mut value = depth.get();
            value.paint += usize::from(guard.paint);
            value.deferred += usize::from(guard.deferred);
            depth.set(value);
        });
        guard
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        DEPTH.with(|depth| {
            let mut value = depth.get();
            value.paint = value.paint.saturating_sub(usize::from(self.paint));
            value.deferred = value.deferred.saturating_sub(usize::from(self.deferred));
            depth.set(value);
        });
    }
}

// Lazy subtrees must restore both boundaries, rather than treating every
// captured paint context as deferred GPUI drawing.
pub(crate) struct DepthScope(Depth);

impl DepthScope {
    pub(crate) fn enter(depth: Depth) -> Self {
        Self(DEPTH.with(|current| current.replace(depth)))
    }
}

impl Drop for DepthScope {
    fn drop(&mut self) {
        DEPTH.with(|current| current.set(self.0));
    }
}
