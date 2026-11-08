//! Measure functions of a far block's leaves kept across frames shrink to
//! what a new measure reads (`far`): a leaf built while a block is recorded
//! offers its compactable state, and the block compacts it once it is known
//! to be far, before its functions are kept for the next pass.

use gpui::Window;
use std::cell::Cell;
use std::rc::Rc;

/// State of a measure function that can drop what a new measure does not read.
pub(crate) trait Compact {
    fn compact(&self);
}

/// Compactable states offered by the leaves of one recorded block.
#[derive(Default)]
pub(crate) struct Compacts(Cell<Vec<Rc<dyn Compact>>>);

impl Compacts {
    /// Offer `state` to the block being recorded.
    pub(crate) fn offer(&self, state: Rc<dyn Compact>) {
        let mut offered = self.0.take();
        offered.push(state);
        self.0.set(offered);
    }

    /// Compact every offered state and let them go.
    pub(super) fn compact_all(&self) {
        for state in self.0.take() {
            state.compact();
        }
    }
}

/// The open block whose leaves may offer compactable states.
pub(crate) fn open(window: &Window) -> Option<Rc<Compacts>> {
    window.render_context::<Compacts>()
}

/// Run `f` with the leaves it builds offering their states to `compacts`.
pub(super) fn with<R>(
    compacts: Rc<Compacts>,
    window: &mut Window,
    f: impl FnOnce(&mut Window) -> R,
) -> R {
    window.with_render_context(Some(compacts), f)
}
