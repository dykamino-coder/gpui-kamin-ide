//! Один CSS paint pass: Appendix E, atomic export и непрозрачные границы.
//! GPUI хранит только окружение и вызывает обход внутри прежних scopes.

mod capture;
mod children;
mod collector;
mod deferred;
mod queue;
mod recording;
mod root;
pub(crate) mod segments;
mod state;

pub(crate) use capture::{LineCapture, capture, replay};
pub(crate) use children::{atomic, box_policy};
pub(crate) use collector::PaintCollect;
pub(crate) use deferred::{PaintInline, PaintLast, REPLAY};
pub(crate) use recording::with_late_jobs;
pub(crate) use root::roots;

use gpui::{PaintPassId, Window};
use recording::recording;
use state::PaintJobQueue;
use std::{
    cell::Cell,
    rc::{Rc, Weak},
};

struct CssPaintPass {
    id: PaintPassId,
    failed: Cell<bool>,
    jobs: PaintJobQueue,
    /// Окружение, общее у соседних строковых заданий, и состояния их повторов
    /// (`gpui::RasterCaptures`): живут, пока открыт корневой кадр прохода.
    raster: gpui::RasterCaptures,
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
        self.open_reserving(window, 0)
    }

    /// Кадр, чья очередь строк возьмёт место под `inline_capacity` заданий сразу.
    fn open_reserving(self: &Rc<Self>, window: &Window, inline_capacity: u32) -> FrameGuard {
        let (depth, previous) = self.jobs.open(window.paint_scope_id(), inline_capacity);
        FrameGuard {
            pass: self.clone(),
            level: u32::try_from(depth).expect("CSS frame depth fits u32"),
            previous,
        }
    }

    fn abort(&self) {
        self.failed.set(true);
        self.jobs.clear();
        self.raster.clear();
    }
}

/// Глубина — `u32`: страж с флагом занимает два слова, и общее состояние пары
/// `PaintCollect` не растёт (сборка держит его на каждый сборщик).
struct FrameGuard {
    pass: Rc<CssPaintPass>,
    level: u32,
    previous: bool,
}

impl FrameGuard {
    fn depth(&self) -> usize {
        self.level as usize
    }
}

impl Drop for FrameGuard {
    fn drop(&mut self) {
        if self.pass.failed.get() {
            return;
        }
        if std::thread::panicking() {
            self.pass.abort();
        } else {
            self.pass.jobs.close(self.depth(), self.previous);
            // Корневой кадр закрыт: заданий прохода больше нет, их окружение не нужно.
            if self.level == 0 {
                self.pass.raster.clear();
            }
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

/// Место очередей строк кадров открытого прохода, от внешнего к внутреннему.
#[cfg(test)]
pub(crate) fn inline_capacities(window: &Window) -> Vec<usize> {
    current(window).map_or_else(Vec::new, |pass| pass.jobs.inline_capacities())
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
