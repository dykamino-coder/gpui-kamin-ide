//! Classic scrollbar visibility is computed from CSS overflow, not decoration bounds.
//! Blink PaintLayerScrollableArea::UpdateAfterLayout relayouts after auto bars change.
use super::{CURRENT, Frame};
use crate::style::computed::{Computed, Overflow};
use gpui::{Bounds, Pixels};

pub(crate) fn model(id: u64, c: &Computed) -> super::super::scrollbar_gutter::Model {
    let frame = CURRENT
        .with(|slot| slot.borrow().clone())
        .filter(|f| f.root.get() == Some(id));
    let cached = frame.as_ref().map_or([false; 2], |f| f.bars.get());
    let overflow = [c.overflow_x, c.overflow_y];
    let bars = std::array::from_fn(|axis| {
        frame.is_some()
            && overflow[axis] == Some(Overflow::Scroll)
            && (c.scroll.forced[axis] || cached[axis])
    });
    let model = super::super::scrollbar_gutter::Model::new(c, bars);
    if let Some(frame) = frame {
        frame
            .reverse
            .set(super::super::scrollbar_gutter::reversed(c));
        frame.reserved.set(model.total());
        frame.insets.set(model.used);
        frame.native_reserved.set(model.native_total());
        frame.bars.set(bars);
    }
    model
}
pub(crate) fn reserve(id: u64, c: &Computed) -> [bool; 2] {
    model(id, c).bars
}

pub(super) fn update(frame: &Frame, c: &Computed, view: [f32; 2]) -> bool {
    let overflow = [c.overflow_x, c.overflow_y];
    let bars = std::array::from_fn(|axis| {
        overflow[axis] == Some(Overflow::Scroll)
            && (c.scroll.forced[axis]
                || view.into_iter().all(|v| v > 0.0) && frame.range.get()[axis] > 0.0)
    });
    frame.bars.replace(bars) != bars
}

pub(super) fn decoration(child: Bounds<Pixels>, owner: Bounds<Pixels>, gutter: [f32; 2]) -> bool {
    let near = |a: Pixels, b: Pixels| (a - b).abs() <= gpui::px(0.81);
    near(child.left(), owner.left())
        && near(child.top(), owner.top())
        && near(child.right(), owner.right() - gpui::px(gutter[0]))
        && near(child.bottom(), owner.bottom() - gpui::px(gutter[1]))
}
