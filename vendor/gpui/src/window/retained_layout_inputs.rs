//! Keyed state used during request_layout belongs to the recorded subtree even
//! when layout precedes its prepaint range. Scroll positions can change between
//! frames without dispatching an input event.

use super::Window;
use crate::{Pixels, Point, ScrollHandle};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub(super) struct ScrollInput {
    offset: Rc<RefCell<Point<Pixels>>>,
    position: Point<Pixels>,
    handle: Option<ScrollHandle>,
}

/// Inputs of the layout that produced a retained subtree in this window.
#[derive(Default)]
pub struct RetainedLayoutInputs {
    states: Cell<(usize, usize)>,
    scroll: Vec<ScrollInput>,
}

impl RetainedLayoutInputs {
    /// Each native scroller still has the position the recorded layout saw.
    pub fn scroll_unchanged(&self) -> bool {
        self.scroll.iter().all(|input| {
            *input.offset.borrow() == input.position
                && !input
                    .handle
                    .as_ref()
                    .is_some_and(ScrollHandle::has_pending_scroll)
        })
    }
}

impl Window {
    /// Capture the keyed state reads and native scroll inputs of this layout.
    pub fn record_layout_inputs<R>(
        &mut self,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> (R, RetainedLayoutInputs) {
        let start = self.next_frame.accessed_element_states.len();
        let outer = self.layout_scroll_inputs.replace(Vec::new());
        let (result, captured) = self.with_restore(
            outer,
            |window, outer| {
                std::mem::swap(&mut window.layout_scroll_inputs, outer);
            },
            layout,
        );
        let scroll = captured.unwrap_or_default();
        if let Some(outer) = &mut self.layout_scroll_inputs {
            outer.extend_from_slice(&scroll);
        }
        (
            result,
            RetainedLayoutInputs {
                states: Cell::new((start, self.next_frame.accessed_element_states.len())),
                scroll,
            },
        )
    }

    pub(crate) fn record_scroll_input(
        &mut self,
        offset: &Rc<RefCell<Point<Pixels>>>,
        handle: Option<&ScrollHandle>,
    ) {
        if let Some(inputs) = &mut self.layout_scroll_inputs {
            inputs.push(ScrollInput {
                position: *offset.borrow(),
                offset: offset.clone(),
                handle: handle.cloned(),
            });
        }
    }

    /// Keep only this laid-out subtree's keyed states on replay. Advance the
    /// range to its new location so consecutive replays retain the same states.
    pub fn replay_layout_inputs(&mut self, inputs: &RetainedLayoutInputs) {
        let (start, end) = inputs.states.get();
        let states = &mut self.next_frame.accessed_element_states;
        let next = states.len();
        states.extend_from_slice(&self.rendered_frame.accessed_element_states[start..end]);
        inputs.states.set((next, states.len()));
        if let Some(outer) = &mut self.layout_scroll_inputs {
            outer.extend_from_slice(&inputs.scroll);
        }
    }
}
