//! Один CSS paint pass: Appendix E, atomic export и непрозрачные границы.
//! GPUI хранит только окружение и вызывает обход внутри прежних scopes.

mod children;
mod collector;
mod deferred;
mod queue;
mod root;

pub(crate) use children::{atomic, box_policy};
pub(crate) use collector::PaintCollect;
pub(crate) use deferred::{PaintInline, PaintLast};
pub(crate) use root::roots;

use gpui::{PaintPassId, PaintScopeId, Window};
use queue::Frame;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
};

#[derive(Default)]
struct State {
    open: bool,
    saved: Vec<bool>,
    frames: Vec<Frame>,
}

struct CssPaintPass {
    id: PaintPassId,
    failed: Cell<bool>,
    state: RefCell<State>,
}

/// Снимок окна удерживает только слабую ссылку: jobs не удерживают свою очередь.
struct Current(Weak<CssPaintPass>);

fn current(window: &Window) -> Option<Rc<CssPaintPass>> {
    window
        .render_context::<Current>()?
        .0
        .upgrade()
        .filter(|pass| pass.id.is_live() && pass.id == window.paint_pass_id() && !pass.failed.get())
}

impl CssPaintPass {
    fn is_open(&self, window: &Window) -> bool {
        let state = self.state.borrow();
        state.open
            && state
                .frames
                .last()
                .is_some_and(|frame| frame.scope == window.paint_scope_id())
    }

    fn open(self: &Rc<Self>, window: &Window) -> FrameGuard {
        let mut state = self.state.borrow_mut();
        let depth = state.frames.len();
        let previous = state.open;
        state.frames.push(Frame::new(window.paint_scope_id()));
        state.open = true;
        FrameGuard {
            pass: self.clone(),
            depth,
            previous,
        }
    }

    fn abort(&self) {
        self.failed.set(true);
        let abandoned = std::mem::take(&mut *self.state.borrow_mut());
        drop(abandoned);
    }
}

struct FrameGuard {
    pass: Rc<CssPaintPass>,
    depth: usize,
    previous: bool,
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        if self.pass.failed.get() {
            return;
        }
        if std::thread::panicking() {
            self.pass.abort();
        } else {
            let mut state = self.pass.state.borrow_mut();
            state.frames.truncate(self.depth);
            state.open = self.previous;
        }
    }
}

struct Boundary {
    pass: Rc<CssPaintPass>,
    previous: bool,
    saved: usize,
}

impl Drop for Boundary {
    fn drop(&mut self) {
        if self.pass.failed.get() {
            return;
        }
        if std::thread::panicking() {
            self.pass.abort();
        } else {
            let mut state = self.pass.state.borrow_mut();
            state.open = self.previous;
            state.saved.truncate(self.saved);
        }
    }
}

fn boundary(window: &mut Window) -> Box<dyn std::any::Any> {
    let Some(pass) = current(window).filter(|pass| pass.is_open(window)) else {
        return Box::new(());
    };
    let (previous, saved) = {
        let mut state = pass.state.borrow_mut();
        let previous = state.open;
        let saved = state.saved.len();
        state.saved.push(previous);
        state.open = false;
        (previous, saved)
    };
    Box::new(Boundary {
        pass,
        previous,
        saved,
    })
}

/// Явный адаптер восстанавливает collector через непрозрачного GPUI consumer.
pub(crate) fn reopen<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    let guard = current(window).map(|pass| {
        let (previous, saved) = {
            let mut state = pass.state.borrow_mut();
            let previous = state.open;
            state.open = state.saved.last().copied().unwrap_or(false);
            (previous, state.saved.len())
        };
        Boundary {
            pass,
            previous,
            saved,
        }
    });
    let result = f(window);
    drop(guard);
    result
}
