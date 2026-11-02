//! Измеренные зависимости пробы: транзакция публикует распорки только при commit.

use super::{lines::Lines, serial::Serial};
use crate::layout::fragment::{
    flow::blocks::MeasuredBreaks,
    push::Growth,
    types::{Kid, StackAxis, StackChild},
};
use gpui::Window;

pub(super) enum Model {
    Lines(Lines),
    Serial(Serial),
}

impl Model {
    pub(super) fn measure(c: &StackChild, axis: StackAxis, window: &mut Window) -> Option<Self> {
        Lines::measure(c, axis, window)
            .map(Self::Lines)
            .or_else(|| Serial::measure(c, axis, window).map(Self::Serial))
    }

    pub(super) fn reset(&mut self) {
        match self {
            Self::Lines(l) => l.reset(),
            Self::Serial(s) => s.reset(),
        }
    }

    pub(super) fn state(&self) -> Vec<u32> {
        match self {
            Self::Lines(l) => l.extra.iter().map(|v| v.to_bits()).collect(),
            Self::Serial(s) => s.state(),
        }
    }

    pub(super) fn accepts(&self, grows: &[Growth]) -> bool {
        match self {
            Self::Lines(l) => l.accepts(grows),
            Self::Serial(s) => s.accepts(grows),
        }
    }

    pub(super) fn grow(&mut self, child: &mut StackChild, base: &Kid, grows: Vec<Growth>) {
        match self {
            Self::Lines(l) => l.grow(child, base, grows),
            Self::Serial(s) => s.grow(child, base, grows),
        }
    }

    pub(super) fn commit(&self, child: &StackChild) -> bool {
        match self {
            Self::Lines(l) => l.commit(),
            Self::Serial(s) => s.commit(child),
        }
    }

    #[cfg(test)]
    pub(super) fn uncommitted(&self) -> bool {
        match self {
            Self::Lines(l) => l.pushes.iter().all(|p| p.growth.borrow().is_empty()),
            Self::Serial(s) => s.uncommitted(),
        }
    }
}

pub(super) fn record(child: &mut StackChild, breaks: MeasuredBreaks) {
    child.cuts = breaks.cuts;
    child.forced = breaks.forced;
    child.solid = breaks.solid;
    child.avoid = breaks.avoid;
    child.keep = breaks.keep;
    child.line_keep = breaks.line_keep;
    child.table_rows = breaks.table_rows;
}
