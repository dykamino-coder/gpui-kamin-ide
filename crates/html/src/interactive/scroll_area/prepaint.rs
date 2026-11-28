//! Scrollport and scrollbar geometry share one box for input and paint.

use super::*;
use gpui::{Bounds, Element, LayoutId, Pixels, Window};

impl ScrollArea {
    pub(super) fn paint_bounds(
        &self,
        bounds: Bounds<Pixels>,
        layout: LayoutId,
        window: &mut Window,
    ) -> Bounds<Pixels> {
        // Match CSS border painting: inset the unrounded box, not a rounded
        // border box. A relocated prepaint_at node must retain its new bounds.
        let exact = Bounds {
            origin: window.layout_origin_unrounded(layout),
            size: window.layout_size_unrounded(layout),
        };
        let half = gpui::px(0.5 / window.scale_factor().max(0.01) + 1e-4);
        let near = |a: Pixels, b: Pixels| (a - b).abs() <= half;
        if near(exact.left(), bounds.left())
            && near(exact.top(), bounds.top())
            && near(exact.right(), bounds.right())
            && near(exact.bottom(), bounds.bottom())
        {
            exact
        } else {
            bounds
        }
    }

    pub(super) fn prepaint_bars(
        &self,
        child: &<Self as Element>::RequestLayoutState,
        paint_bounds: Bounds<Pixels>,
        window: &mut Window,
    ) -> Vec<crate::interactive::scrollbars::interaction::HitBar> {
        let bars = RootBars {
            style: self.style.clone(),
            freeze: self.freeze_scrollbars,
            handle: child.1.clone(),
            extent: child.2.clone(),
            interaction: child.4.clone(),
            bounds: paint_bounds,
            animation: child.5.clone(),
        };
        // The root scrollport's bars follow the offset: the document's frame
        // element draws them after its record each frame (`RootBars`).
        match self.root.as_ref().filter(|root| root.hosted.get()) {
            Some(root) => {
                root.bars.replace(Some(Rc::new(bars)));
                Vec::new()
            }
            None => bars.prepaint(window),
        }
    }
}
