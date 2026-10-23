//! Щуп статической позиции `Probe`: в `prepaint` записывает в `SpotCell` неокруглённую
//! геометрию CSS дырки на месте абсолюта (с поворотом вертикального письма, строчной
//! коробкой и координатами области просмотра), чтобы позиционированное поддерево встало
//! по ней позже, когда его разложит содержащий блок.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::positioned::containing_block::SpotCell;
use crate::text::vertical::VerticalPaint;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};
use std::rc::Rc;

pub(super) fn probe(
    spot: SpotCell,
    full: bool,
    line: Option<crate::layout::positioned::containing_block::LineBoxSlot>,
    child: AnyElement,
    paint_context: Rc<VerticalPaint>,
    fragments: Rc<crate::layout::cx::deferred::Deferred>,
) -> AnyElement {
    Probe {
        paint_context,
        fragments,
        spot,
        full,
        line,
        child,
    }
    .hooked()
}

struct Probe {
    paint_context: Rc<VerticalPaint>,
    fragments: Rc<crate::layout::cx::deferred::Deferred>,
    spot: SpotCell,
    full: bool,
    /// The line box of the text line this probe sits in, when a paragraph
    /// places it (`LineBoxSlot`).
    line: Option<crate::layout::positioned::containing_block::LineBoxSlot>,
    child: AnyElement,
}

impl Hooks for Probe {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let id = self.child.request_layout(window, cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(*id),
            size: window.layout_size_unrounded(*id),
        };
        let mut now = self.spot.get();
        now.rotated = self.paint_context.is_rotated();
        now.block_strut = self.full;
        if !self.full {
            now.line_box = self.line.as_ref().and_then(|slot| slot.take());
        }
        let hole = self.paint_context.map(bounds, px(now.line_thickness));
        now.hole = Some(if now.viewport_static {
            crate::layout::fragment::static_coordinates::map(&self.fragments, hole)
        } else {
            hole
        });
        self.spot.set(now);
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut Self::Layout,
        _: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
