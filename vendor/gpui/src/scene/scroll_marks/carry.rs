//! KaminIDE patch: the marks of the scroll nodes a paint record opened inside
//! itself, carried to its copy where it lands (`Window::paint_plan`).
//!
//! A node opened while a record records paints into the record's scene:
//! its commands are marked there, under it, and its ranges index that scene.
//! The copy landing in the frame keeps those marks, and tells where each
//! command landed, so the node's ranges move into the frame's scene and a
//! scrolled replay moves the node as one it painted itself.

use super::*;

/// The marks still to carry, in command order, and where the commands of
/// the source landed (`u32::MAX`: dropped by the copy).
#[derive(Default)]
pub(crate) struct Carry {
    marks: Vec<ScrollMark>,
    next: usize,
    pub(crate) landed: Vec<u32>,
}

impl Carry {
    /// Carry the marks of `source`, a finished record's scene.
    pub(crate) fn of(source: &Scene) -> Box<Self> {
        Box::new(Carry { marks: source.scroll.marks.clone(), next: 0, landed: Vec::with_capacity(source.paint_operations.len()) })
    }

    /// The mark of source command `index`, taken in order.
    pub(super) fn take(&mut self, index: usize) -> Option<ScrollMark> {
        let mark = self.marks.get(self.next).filter(|mark| mark.op as usize == index).cloned();
        self.next += mark.is_some() as usize;
        mark
    }

    /// Where `range` of the source landed: all its commands kept, in order.
    /// An empty range lands before the first command kept after it.
    pub(crate) fn range(&self, range: &Range<usize>) -> Option<Range<usize>> {
        let kept = self.landed.get(range.clone())?;
        if kept.iter().any(|&at| at == u32::MAX) {
            return None;
        }
        let start = match kept.first() {
            Some(&at) => at as usize,
            None => {
                let after = self.landed.get(range.start..)?.iter().find(|&&at| at != u32::MAX).copied();
                let before = self.landed[..range.start].iter().rev().find(|&&at| at != u32::MAX).map(|&at| at + 1);
                after.or(before)? as usize
            }
        };
        Some(start..start + kept.len())
    }
}

impl Scene {
    /// The carried mark of source command `index` (`replay_shifted`).
    pub(in crate::scene) fn carried(&mut self, index: usize) -> Option<ScrollMark> {
        self.scroll.carry.as_mut().and_then(|carry| carry.take(index))
    }

    /// Note where the source command just handled landed (`None`: dropped),
    /// and give it its carried mark, raised as the copy raised its orders.
    pub(in crate::scene) fn land_carried(&mut self, at: Option<usize>, mark: Option<ScrollMark>, raise: DrawOrder) {
        let Some(carry) = self.scroll.carry.as_mut() else { return };
        carry.landed.push(at.map_or(u32::MAX, |at| at as u32));
        if let (Some(at), Some(mark)) = (at, mark) {
            let dormant = matches!(self.paint_operations.get(at), Some(PaintOperation::Dormant(_)));
            let raised = mark.raised + if dormant { raise } else { 0 };
            self.scroll.marks.push(ScrollMark { op: at as u32, raised, ..mark });
        }
    }
}
