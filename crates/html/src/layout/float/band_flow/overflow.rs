//! Keep normal-flow height separate from the scrollable overflow of floated children.

use super::Slot;
use gpui::{MeasuredContent, px, size};

#[derive(Clone, Debug, Default)]
pub(super) struct Plan {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) slots: Vec<Slot>,
}

impl Plan {
    pub(super) fn measured(&self, vert: Option<bool>, inline_up: bool) -> MeasuredContent {
        let mut measured = MeasuredContent::new(match vert {
            None => size(px(self.width), px(self.height)),
            Some(_) => size(px(self.height), px(self.width)),
        });
        let mut extent = measured.size;
        self.extend(&self.slots, vert, inline_up, &mut extent);
        measured.overflow = Some(extent);
        measured
    }

    fn extend(
        &self,
        slots: &[Slot],
        vert: Option<bool>,
        inline_up: bool,
        extent: &mut gpui::Size<gpui::Pixels>,
    ) {
        for slot in slots {
            // Slots, including nested slots, use host coordinates, as do paint holders.
            let (left, top, width, height) = match vert {
                None => (slot.x, slot.y, slot.i, slot.b),
                Some(true) => (self.height - slot.y - slot.b, slot.x, slot.b, slot.i),
                Some(false) => (
                    slot.y,
                    if inline_up {
                        self.width - slot.x - slot.i
                    } else {
                        slot.x
                    },
                    slot.b,
                    slot.i,
                ),
            };
            let overflow = slot.overflow;
            if slot.h.is_some() {
                extent.width = extent.width.max(px(left + width));
                extent.height = extent.height.max(px(top + height));
            }
            extent.width = extent.width.max(px(left) + overflow.right());
            extent.height = extent.height.max(px(top) + overflow.bottom());
            // Nest shares this BFC and cannot clip its children (band_nest_block).
            self.extend(&slot.kids, vert, inline_up, extent);
        }
    }
}
