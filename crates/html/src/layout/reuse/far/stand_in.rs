//! Stand-in of a far block: claims the block's kept layout subtree (`super`).

use super::FarSlot;
use crate::hooks::Hooks;
use gpui::{App, Bounds, LayoutId, Pixels, Window};
use std::rc::Rc;

/// Stand-in of a far block: its claimed layout subtree, no prepaint or paint.
pub(in crate::layout::reuse) struct Far {
    slot: Rc<FarSlot>,
}

impl Far {
    pub(in crate::layout::reuse) fn new(slot: Rc<FarSlot>) -> Self {
        Self { slot }
    }
}

impl Hooks for Far {
    fn paint_boundary(&self) -> bool {
        false
    }

    /// A leaf with no prepaint: nothing to dispatch to, as `recorder::Recorder` is.
    fn dispatch_node(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut subtree = self.slot.subtree.get();
        match window.request_retained_layout(&mut subtree) {
            Some(id) => {
                self.slot.subtree.set(subtree);
                self.slot.layout.set(Some(id));
                // The kept leaves make their probes here, where the build of
                // the block would have requested them, unless the widths are
                // those of the last pass (Taffy's cache, no measure).
                if self.slot.prepare {
                    window.prepare_retained_measures(&subtree);
                }
                (id, ())
            }
            // The nodes are gone: the frame check refuses the block.
            None => (window.request_layout(gpui::Style::default(), [], cx), ()),
        }
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        // Painted means the frame check kept the block: the next pass may
        // claim it again with the same functions, shrunk to what a new
        // measure reads (`gpui::CompactMeasure`).
        if self.slot.layout.get().is_some() {
            window.keep_retained_measures(&self.slot.subtree.get(), true);
        }
    }
}
