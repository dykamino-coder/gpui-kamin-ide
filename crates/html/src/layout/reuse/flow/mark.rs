//! Состояние сборки до блочного ребёнка корневого потока (`FlowCx::mark`).

use super::super::Counters;
use crate::layout::options::RenderOpts;
use std::rc::Rc;

/// Состояние сборки до блочного ребёнка: его сборка должна изменить только
/// счётчики и добавить ровно один элемент.
pub(crate) struct Mark {
    pub(super) start: Counters,
    pub(super) out: usize,
    pub(super) outlines: usize,
    pub(super) coordinates: usize,
    pub(super) late: bool,
    /// Чтения ширин снаружи сборкой ребёнка (`BoxConstraints::track_widths`).
    pub(super) widths: Option<Rc<std::cell::Cell<bool>>>,
}

impl Mark {
    /// Опции сборки ребёнка: с записью чтений ширин, если его записывают.
    pub(crate) fn child_opts(&self, opts: &RenderOpts) -> Option<Box<RenderOpts>> {
        let sink = self.widths.clone()?;
        Some(opts.inner(|cx| cx.constraints.track_widths(sink)))
    }
}

pub(super) fn mark_of(opts: &RenderOpts, out: usize) -> Mark {
    let paint = &opts.cx.deferred.paint;
    Mark {
        start: Counters::read(&opts.cx),
        out,
        outlines: paint.outlines.borrow().len(),
        coordinates: paint.coordinates.borrow().len(),
        late: opts.cx.position.state.late_pending(),
        widths: None,
    }
}
