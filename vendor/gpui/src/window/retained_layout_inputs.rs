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

    /// The first scroll read whose position changed or whose handle has a
    /// pending scroll (`scroll_unchanged` false), for the `why` journal: its
    /// node, what the layout read and what it is now.
    pub fn scroll_change(&self) -> Option<String> {
        self.scroll.iter().enumerate().find_map(|(at, input)| {
            let now = *input.offset.borrow();
            let pending = input.handle.as_ref().is_some_and(ScrollHandle::has_pending_scroll);
            (now != input.position || pending).then(|| {
                let node = input.handle.as_ref().map(super::ScrollNodeId::of);
                format!(
                    "scroll read {at} of {} (node {node:?}): read {:?}, now {now:?}, pending scroll {pending}",
                    self.scroll.len(),
                    input.position
                )
            })
        })
    }
}

impl Window {
    /// A changed layout offset can replay only if the same native scroll handle
    /// belongs to this recorded range. Its node then supplies the clip and shift.
    /// A pending scroll-to-item still needs layout to resolve its destination.
    pub fn layout_scroll_replayable(&self, inputs: &RetainedLayoutInputs, record: &super::RetainedPrepaint) -> bool {
        let range = record.range();
        inputs.scroll.iter().all(|input| {
            let handle = input.handle.as_ref();
            if handle.is_some_and(ScrollHandle::has_pending_scroll) {
                return false;
            }
            *input.offset.borrow() == input.position || handle.is_some_and(|handle| {
                let id = super::ScrollNodeId::of(handle);
                self.rendered_frame.scroll_nodes[range.start.scroll_nodes_index..range.end.scroll_nodes_index]
                    .iter()
                    .any(|node| node.id == id)
            })
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px};

    /// DevTools names the scroll read that keeps a record from replaying.
    #[test]
    fn a_changed_scroll_read_is_named() {
        let offset = Rc::new(RefCell::new(point(px(0.), px(-40.))));
        let read = ScrollInput { offset: offset.clone(), position: point(px(0.), px(-40.)), handle: None };
        let inputs = RetainedLayoutInputs { states: Cell::default(), scroll: vec![read] };
        assert!(inputs.scroll_unchanged() && inputs.scroll_change().is_none());
        *offset.borrow_mut() = point(px(0.), px(0.));
        let change = inputs.scroll_change().expect("the read changed");
        assert!(!inputs.scroll_unchanged());
        assert!(change.starts_with("scroll read 0 of 1 (node None): read"), "{change}");
        assert!(change.contains("pending scroll false"), "{change}");
    }
}
