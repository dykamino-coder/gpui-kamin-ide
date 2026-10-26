//! Один CSS paint pass: Appendix E, atomic export и непрозрачные границы.
//! GPUI хранит только окружение и вызывает обход внутри прежних scopes.

mod children;
mod collector;
mod deferred;
mod queue;
mod root;
mod state;

pub(crate) use children::{atomic, box_policy};
pub(crate) use collector::PaintCollect;
pub(crate) use deferred::{PaintInline, PaintLast};
pub(crate) use root::roots;

use gpui::{PaintPassId, Window};
use state::PaintJobQueue;
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

struct CssPaintPass {
    id: PaintPassId,
    failed: Cell<bool>,
    jobs: PaintJobQueue,
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
        self.jobs.is_open(window.paint_scope_id())
    }

    fn open(self: &Rc<Self>, window: &Window) -> FrameGuard {
        let (depth, previous) = self.jobs.open(window.paint_scope_id());
        FrameGuard {
            pass: self.clone(),
            depth,
            previous,
        }
    }

    fn abort(&self) {
        self.failed.set(true);
        self.jobs.clear();
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
            self.pass.jobs.close(self.depth, self.previous);
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
            self.pass.jobs.restore(self.previous, self.saved);
        }
    }
}

fn boundary(window: &mut Window) -> Box<dyn std::any::Any> {
    let Some(pass) = current(window).filter(|pass| pass.is_open(window)) else {
        return Box::new(());
    };
    let (previous, saved) = pass.jobs.enter_boundary();
    Box::new(Boundary {
        pass,
        previous,
        saved,
    })
}

/// Явный адаптер восстанавливает collector через непрозрачного GPUI consumer.
pub(crate) fn reopen<R>(window: &mut Window, f: impl FnOnce(&mut Window) -> R) -> R {
    let guard = current(window).map(|pass| {
        let (previous, saved) = pass.jobs.reopen();
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
