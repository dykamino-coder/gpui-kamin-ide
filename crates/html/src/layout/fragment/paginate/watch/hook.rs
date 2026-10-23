//! Обёртка `Watch` наблюдаемой коробки: записывает внутренний узел раскладки в рекордер
//! сборки (`Watched::seen`) и отдаёт его как свой.

use super::{TablePart, Watched};
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, ContentMask, LayoutId, Pixels, Window, px};
use std::rc::Rc;

/// Wrap `el` when the build's recorder watches `id` (identity otherwise).
pub(crate) fn wrap(id: u64, el: AnyElement, opts: &RenderOpts) -> AnyElement {
    match &opts.frag.watch {
        Some(rec) if rec.ids.contains(&id) => Watch {
            slot: Slot::Box(id),
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

/// What a watched node is recorded as.
#[derive(Clone, Copy)]
pub(super) enum Slot {
    /// Border box of the box with this node id.
    Box(u64),
    /// Part of the table with this key (`table_part`).
    Table(u64, TablePart),
}

pub(super) struct Watch {
    pub(super) slot: Slot,
    pub(super) inner: AnyElement,
    pub(super) rec: Rc<Watched>,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = Option<ContentMask<Pixels>>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        match self.slot {
            Slot::Box(id) => {
                if let Some(growth) = self.rec.flex_growth.borrow().get(&id) {
                    window.set_flex_line_fragmentation_growth(lid, growth);
                }
                if let Some(&line) = self.rec.flex_lines.borrow().get(&id) {
                    window.set_flex_item_fragmentation_line(lid, Some(line));
                }
                if let Some(offset) = self.rec.flex_offsets.borrow().get(&id) {
                    window.set_flex_item_fragmentation_offset(lid, *offset);
                }
                if let Some(consumed) = self.rec.flex_consumed.borrow().get(&id) {
                    window.set_flex_item_fragmentation_consumed(lid, consumed.0);
                }
                self.rec.seen.borrow_mut().insert(id, lid);
            }
            Slot::Table(key, part) => self.rec.record_part(key, part, lid),
        }
        (lid, ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let mask = if let Slot::Box(id) = self.slot {
            self.rec
                .flex_consumed
                .borrow()
                .get(&id)
                .map(|&(consumed, complete)| {
                    let mut mask = window.content_mask();
                    let bottom = mask.bounds.bottom();
                    let top = if complete {
                        bottom
                    } else {
                        mask.bounds.origin.y.max(bounds.origin.y + px(consumed))
                    };
                    mask.bounds.origin.y = top;
                    mask.bounds.size.height = (bottom - top).max(px(0.));
                    mask
                })
        } else {
            None
        };
        window.with_content_mask(mask, |window| self.inner.prepaint(window, cx));
        mask
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let access = match self.slot {
            Slot::Box(id) => self.rec.access(id, window.content_mask()),
            Slot::Table(..) => super::owner::Access::Normal,
        };
        match access {
            super::owner::Access::Hidden => {}
            super::owner::Access::Escape(mask) => window
                .with_content_mask_replaced(mask, |window| {
                    window.with_content_mask(*prepaint, |window| self.inner.paint(window, cx))
                }),
            super::owner::Access::Normal => {
                window.with_content_mask(*prepaint, |window| self.inner.paint(window, cx))
            }
        }
    }
}
