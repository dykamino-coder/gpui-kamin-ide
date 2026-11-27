//! KaminIDE patch: the marks of the scroll nodes a paint record opened inside
//! itself, carried to its copy where it lands (`Window::paint_plan`).
//!
//! A node opened while a record records paints into the record's scene:
//! its commands are marked there, under it, and its ranges index that scene.
//! The copy landing in the frame keeps those marks, moved and clipped as the
//! copy is, and tells where each command landed, so the node's ranges move
//! into the frame's scene and a scrolled replay moves the node as one it
//! painted itself. A command the landing clip hides at every offset of its
//! node is gone, as `mark_scrolled` drops one outside its own mask.

use super::*;

/// A source command the copy dropped, though its node may show it.
const DROPPED: u32 = u32::MAX;
/// A source command its node can no longer show where the copy lands.
const GONE: u32 = u32::MAX - 1;

/// The scene a record's nodes were painted in and the landings that copied
/// its commands since, in order, into a scene that lands later itself.
pub(crate) type Relay = (SceneSpace, Vec<Box<Carry>>);

/// The marks still to carry, in command order, where the copy lands
/// (`offset`, `clip`), and where the commands of the source landed.
#[derive(Clone)]
pub(crate) struct Carry {
    marks: Vec<ScrollMark>,
    next: usize,
    offset: Point<ScaledPixels>,
    clip: Option<Bounds<ScaledPixels>>,
    pub(crate) landed: Vec<u32>,
    /// The nodes of the carried marks the copy kept (repeated per mark).
    pub(crate) kept: Vec<ScrollNodeId>,
}

impl Carry {
    /// Carry the marks of `source`, a finished record's scene, landing
    /// moved by `offset` and clipped by `clip`.
    pub(crate) fn of(source: &Scene, offset: Point<ScaledPixels>, clip: Option<Bounds<ScaledPixels>>) -> Box<Self> {
        let landed = Vec::with_capacity(source.paint_operations.len());
        Box::new(Carry { marks: source.scroll.marks.clone(), next: 0, offset, clip, landed, kept: Vec::new() })
    }

    /// Where the copy lands: its offset and clip.
    pub(crate) fn placement(&self) -> (Point<ScaledPixels>, Option<Bounds<ScaledPixels>>) {
        (self.offset, self.clip)
    }

    /// What a later landing of the copy needs: its marks are carried already.
    pub(crate) fn landed(mut self: Box<Self>) -> Box<Self> {
        self.marks = Vec::new();
        self
    }

    /// The mark of source command `index`, taken in order.
    pub(super) fn take(&mut self, index: usize) -> Option<ScrollMark> {
        let mark = self.marks.get(self.next).filter(|mark| mark.op as usize == index).cloned();
        self.next += mark.is_some() as usize;
        mark
    }

    /// Where `range` of the source landed: all its commands kept or gone, in
    /// order. An empty landing lies before the first command kept after it.
    pub(crate) fn range(&self, range: &Range<usize>) -> Option<Range<usize>> {
        let landed = self.landed.get(range.clone())?;
        if landed.contains(&DROPPED) {
            return None;
        }
        let kept = |at: &&u32| **at < GONE;
        let len = landed.iter().filter(kept).count();
        let start = match landed.iter().find(kept) {
            Some(&at) => at as usize,
            None => {
                let after = self.landed.get(range.start..)?.iter().find(kept).copied();
                let before = self.landed[..range.start].iter().rev().find(kept).map(|&at| at + 1);
                after.or(before)? as usize
            }
        };
        Some(start..start + len)
    }
}

impl Scene {
    /// The carried mark of source command `index` (`replay_shifted`).
    pub(in crate::scene) fn carried(&mut self, index: usize) -> Option<ScrollMark> {
        self.scroll.carry.as_mut().and_then(|carry| carry.take(index))
    }

    /// The carried `mark` of source command `index`, which the copy placed
    /// (`shown`: visible there): moved and clipped as the copy, its pivot
    /// moved with it. A primitive the clip hides turns dormant; `None` when
    /// its node can no longer show it there (gone, `land_gone`).
    pub(in crate::scene) fn carried_landing(
        &mut self,
        operation: &mut PaintOperation,
        mark: ScrollMark,
        shown: bool,
        index: usize,
    ) -> Option<ScrollMark> {
        let (offset, clip) = self.scroll.carry.as_ref().map(|carry| (carry.offset, carry.clip))?;
        let moved = |bounds: Bounds<ScaledPixels>| Bounds { origin: bounds.origin + offset, size: bounds.size };
        let own = clip.map_or(moved(mark.own), |clip| moved(mark.own).intersect(&clip));
        let mut mark = ScrollMark { own, placed: moved(mark.placed), ..mark };
        match operation {
            PaintOperation::Primitive(primitive) => {
                if mark.placed.intersect(&own).is_empty() {
                    return None;
                }
                let pivot = mark.pivot.take().filter(|_| offset == Point::default());
                mark.pivot = self.copied_pivot(primitive, index, false).or(pivot);
                if !shown {
                    *operation = PaintOperation::Dormant(std::rc::Rc::new(primitive.clone()));
                }
            }
            // It takes the copy's shift when it wakes, as a scrolled replay's.
            PaintOperation::Dormant(_) if mark.placed.intersect(&own).is_empty() => return None,
            PaintOperation::Dormant(_) => mark.moved = mark.moved + offset,
            _ => {}
        }
        Some(mark)
    }

    /// Note that the source command just handled is gone where it lands.
    pub(in crate::scene) fn land_gone(&mut self) {
        if let Some(carry) = self.scroll.carry.as_mut() {
            carry.landed.push(GONE);
        }
    }

    /// Note where the source command just handled landed (`None`: dropped),
    /// and give it its carried mark, raised as the copy raised its orders.
    pub(in crate::scene) fn land_carried(&mut self, at: Option<usize>, mark: Option<ScrollMark>, raise: DrawOrder) {
        let Some(carry) = self.scroll.carry.as_mut() else { return };
        carry.landed.push(at.map_or(DROPPED, |at| at as u32));
        if let (Some(at), Some(mark)) = (at, mark) {
            carry.kept.push(mark.node);
            let dormant = matches!(self.paint_operations.get(at), Some(PaintOperation::Dormant(_)));
            let raised = mark.raised + if dormant { raise } else { 0 };
            self.scroll.marks.push(ScrollMark { op: at as u32, raised, ..mark });
        }
    }
}