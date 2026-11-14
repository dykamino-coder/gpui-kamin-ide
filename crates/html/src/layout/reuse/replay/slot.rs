//! Место повтора блока в кадре и его проверка перед prepaint.

use super::{Ranges, ReplayRecord, plain_environment};
use crate::hooks::Placement;
use gpui::{Bounds, LayoutId, Pixels, Point, ScaledPixels, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(in crate::layout::reuse) struct ReplaySlot {
    pub(super) record: Rc<ReplayRecord>,
    pub(super) layout: Cell<Option<LayoutId>>,
    /// The block lies at another place than recorded (`valid`): where its
    /// paint goes, and its new origin.
    pub(super) moved: Cell<Option<(Placement, Point<Pixels>)>>,
}

impl ReplaySlot {
    pub(in crate::layout::reuse) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            moved: Cell::new(None),
        }
    }

    /// DOM node of the replayed block.
    pub(in crate::layout::reuse) fn node(&self) -> u64 {
        self.record.source.id()
    }

    /// Повтор этого кадра точен: узлы заняты, ни одна мера не понадобилась,
    /// блок того же размера лёг в том же окружении на записанное место либо
    /// переносим на новое (`moved_to`), а диапазоны записаны в только что
    /// показанном кадре.
    pub(in crate::layout::reuse) fn valid(&self, window: &mut Window) -> bool {
        let Some(id) = self.layout.get() else {
            return false;
        };
        let fresh = !window.layout_stale() && plain_environment(window);
        let (origin, size) = (
            window.layout_origin_unrounded(id),
            window.layout_size_unrounded(id),
        );
        let (serial, scale, mask) = (
            window.frame_serial(),
            window.scale_factor(),
            window.content_mask(),
        );
        let placed = self.record.ranges(|r| {
            let same = r.serial + 1 == serial
                && r.lines.complete()
                && r.scale == scale
                && r.mask == mask
                && r.size == size;
            match same {
                true if r.origin == origin => Some(None),
                true => moved_to(r, origin, window).map(|placed| Some((placed, origin))),
                false => None,
            }
        });
        let Some(placed) = placed.flatten().filter(|_| fresh) else {
            return false;
        };
        self.moved.set(placed);
        true
    }
}

/// Where the paint of the record `r` goes when its block lies at `origin`,
/// another place than recorded, if the paint can be moved there exactly.
///
/// The offset is a whole number of device pixels: glyphs and snapped edges
/// painted afresh at the new place are then the recorded ones moved. All of
/// the block's ink lay inside the frame's mask: a primitive the mask hid is
/// not in the record, and at the new place it might show. And the recorded
/// output is scene commands under that mask alone
/// (`Window::retained_placeable`): nothing in it holds the recorded place.
/// Blink and Gecko paint a moved box again (Blink invalidates its display
/// items for the layout change); here the recorded commands are moved.
///
/// A block with text is not moved yet: its paragraphs register selection
/// listeners that hold the recorded place (interactive::selection).
fn moved_to(r: &Ranges, origin: Point<Pixels>, window: &Window) -> Option<Placement> {
    let mask = r.mask.bounds.scale(r.scale);
    let offset = (origin - r.origin).scale(r.scale);
    let whole = offset.x.0.fract() == 0.0 && offset.y.0.fract() == 0.0;
    let inside = |ink: Bounds<ScaledPixels>| ink.is_contained_within(&mask);
    (whole
        && r.ink.is_none_or(inside)
        && window.retained_placeable(&r.prepaint, &r.paint, mask)
        && r.lines.placeable(window, mask))
    .then_some((offset, mask))
}