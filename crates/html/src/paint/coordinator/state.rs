//! Владелец CSS-очередей и scopes: мутации не пересекают вызовы paint.

use super::{
    CssPaintPass,
    queue::{Frame, Inline, Positioned},
};
use gpui::{PaintPassId, PaintScopeId};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Default)]
struct State {
    open: bool,
    saved: Vec<bool>,
    frames: Vec<Frame>,
}

#[derive(Default)]
pub(super) struct PaintJobQueue(RefCell<State>);

impl PaintJobQueue {
    // Единственная точка мутации. Замыкания — внутренние операции владельца;
    // ни элементы, ни consumers не получают доступ к State или RefCell.
    fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.0.borrow_mut())
    }

    pub fn is_open(&self, scope: PaintScopeId) -> bool {
        let state = self.0.borrow();
        state.open
            && state
                .frames
                .last()
                .is_some_and(|frame| frame.scope == scope)
    }

    pub fn open(&self, scope: PaintScopeId) -> (usize, bool) {
        self.update(|state| {
            let previous = state.open;
            let depth = state.frames.len();
            state.frames.push(Frame::new(scope));
            state.open = true;
            (depth, previous)
        })
    }

    pub fn clear(&self) {
        let abandoned = self.update(std::mem::take);
        drop(abandoned);
    }

    pub fn close(&self, depth: usize, previous: bool) {
        let abandoned = self.update(|state| {
            let abandoned = state.frames.split_off(depth);
            state.open = previous;
            abandoned
        });
        drop(abandoned);
    }

    pub fn enter_boundary(&self) -> (bool, usize) {
        self.update(|state| {
            let previous = state.open;
            let saved = state.saved.len();
            state.saved.push(previous);
            state.open = false;
            (previous, saved)
        })
    }

    pub fn reopen(&self) -> (bool, usize) {
        self.update(|state| {
            let previous = state.open;
            state.open = state.saved.last().copied().unwrap_or(false);
            (previous, state.saved.len())
        })
    }

    pub fn restore(&self, previous: bool, saved: usize) {
        self.update(|state| {
            state.open = previous;
            state.saved.truncate(saved);
        });
    }

    pub fn admit(&self, job: Positioned) {
        self.update(|state| {
            state.frames.last_mut().expect("open CSS frame").push(job);
        });
    }

    pub fn admit_inline(&self, job: Inline) {
        self.update(|state| {
            state
                .frames
                .last_mut()
                .expect("open CSS frame")
                .inline
                .push_back(job);
        });
    }

    pub fn pop_inline(&self, depth: usize) -> Option<Inline> {
        self.update(|state| {
            state
                .frames
                .get_mut(depth)
                .and_then(|frame| frame.inline.pop_front())
        })
    }

    pub fn pop_min(&self, depth: usize) -> Option<Positioned> {
        self.update(|state| state.frames.get_mut(depth).and_then(Frame::next))
    }

    pub fn export_atomic(&self, depth: usize) {
        self.update(|state| {
            let moved = std::mem::take(&mut state.frames[depth].positioned);
            for job in moved.into_values().flatten() {
                state.frames[depth - 1].push(job);
            }
        });
    }
}

/// Соседние прозрачные корни разделяют один живой проход окна.
#[derive(Default)]
pub(super) struct PassOwner(RefCell<Option<Rc<CssPaintPass>>>);

impl PassOwner {
    pub fn select(&self, id: PaintPassId, outer: Option<&Rc<CssPaintPass>>) -> Rc<CssPaintPass> {
        let mut owned = self.0.borrow_mut();
        if owned
            .as_ref()
            .is_none_or(|pass| !pass.id.is_live() || pass.id != id || pass.failed.get())
        {
            *owned = Some(outer.cloned().unwrap_or_else(|| {
                Rc::new(CssPaintPass {
                    id,
                    failed: Cell::new(false),
                    jobs: PaintJobQueue::default(),
                })
            }));
        }
        owned.as_ref().expect("active CSS pass").clone()
    }
}
