//! Static positions retain CSS geometry until the positioned subtree is placed.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::positioned::containing_block::SpotCell;
use crate::text::vertical::{VT_FRAME, vt_map};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

pub(super) fn probe(spot: SpotCell, full: bool, child: AnyElement) -> AnyElement {
    Probe { spot, full, child }.hooked()
}

struct Probe {
    spot: SpotCell,
    full: bool,
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
        now.rotated = VT_FRAME.with(|frame| frame.get()).is_some();
        now.block_strut = self.full;
        if !self.full {
            now.line_box = crate::layout::positioned::containing_block::take_line_box();
        }
        let hole = vt_map(bounds, px(now.line_thickness));
        now.hole = Some(if now.viewport_static {
            crate::layout::fragment::static_coordinates::map(hole)
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
