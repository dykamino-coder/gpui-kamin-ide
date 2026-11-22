//! The bars of a scrollport drawn apart from its content (`RootBars`).

use gpui::{Bounds, Pixels, Window};

/// The bars of a scrollport, drawn apart from its content. The thumbs follow
/// the scroll offset, so the root scrollport's are drawn by the document's
/// frame element after its retained record, every frame: a wheel step that
/// replays the record moved draws them at the new offset (as cc keeps
/// scrollbar layers out of the scrolled content's paint).
pub(crate) struct RootBars {
    pub(super) style: crate::style::computed::BoxStyle,
    pub(super) freeze: bool,
    pub(super) handle: gpui::ScrollHandle,
    pub(super) extent: crate::interactive::scroll_extent::Frame,
    pub(super) interaction: crate::interactive::scrollbars::interaction::Interaction,
    pub(super) bounds: Bounds<Pixels>,
}

impl RootBars {
    /// A diagnostic wheel target in the visible root content, beside its bar.
    #[cfg(feature = "engine-bench")]
    pub(crate) fn wheel_position(&self, window: &Window) -> Option<gpui::Point<Pixels>> {
        let edges = self.style.borders();
        let edge = |v| match v {
            Some(crate::style::values::value::Len::Px(v)) => v,
            _ => 0.0,
        };
        let gutter = self.extent.insets();
        let insets = [
            edge(edges.left),
            edge(edges.top),
            edge(edges.right),
            edge(edges.bottom),
        ];
        let insets = std::array::from_fn(|i| insets[i] + gutter[i]);
        wheel_position(
            self.bounds,
            insets,
            self.handle
                .bounds()
                .intersect(&Bounds::new(gpui::Point::default(), window.viewport_size())),
            crate::interactive::scrollbar_gutter::on_left(&self.style),
            window.scale_factor(),
        )
    }

    /// The hit areas of the bars.
    pub(crate) fn prepaint(
        &self,
        window: &mut Window,
    ) -> Vec<crate::interactive::scrollbars::interaction::HitBar> {
        crate::interactive::scrollbars::follow(
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            window,
        );
        crate::interactive::scrollbars::interaction::prepaint(
            self.freeze,
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            window,
        )
    }

    /// Their listeners and paint at the offset now.
    pub(crate) fn paint(
        &self,
        hitbars: &[crate::interactive::scrollbars::interaction::HitBar],
        window: &mut Window,
    ) {
        let alpha = self
            .interaction
            .paint(self.freeze, hitbars, &self.handle, window);
        crate::interactive::scrollbars::paint(
            &self.style,
            &self.handle,
            self.extent.bars(),
            self.extent.range(),
            self.extent.insets(),
            self.bounds,
            alpha,
            window,
        );
    }
}

/// Native bounds can include replay overscan. Target the visible content box,
/// excluding its borders and reserved gutters, clipped to the native listener.
#[cfg(any(feature = "engine-bench", test))]
fn wheel_position(
    bounds: Bounds<Pixels>,
    insets: [f32; 4],
    clip: Bounds<Pixels>,
    left: bool,
    scale: f32,
) -> Option<gpui::Point<Pixels>> {
    use gpui::{point, px};
    let bounds = Bounds::from_corners(
        point(bounds.left() + px(insets[0]), bounds.top() + px(insets[1])),
        point(
            bounds.right() - px(insets[2]),
            bounds.bottom() - px(insets[3]),
        ),
    )
    .intersect(&clip);
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return None;
    }
    let inset = px(0.5 / scale.max(0.01)).min(bounds.size.width / 2.0);
    Some(point(
        if left {
            bounds.left() + inset
        } else {
            bounds.right() - inset
        },
        bounds.center().y,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn wheel_target_excludes_gutters_and_native_overscan() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0)));
        let native = Bounds::new(
            point(px(-4000.0), px(-4000.0)),
            size(px(8785.0), px(8600.0)),
        );
        assert_eq!(
            wheel_position(bounds, [0.0, 0.0, 15.0, 0.0], native, false, 1.0),
            Some(point(px(784.5), px(300.0)))
        );
        assert_eq!(
            wheel_position(bounds, [15.0, 0.0, 0.0, 0.0], native, true, 2.0),
            Some(point(px(15.25), px(300.0)))
        );
    }

    #[test]
    fn clipped_narrow_and_empty_scrollports_keep_the_target_inside() {
        let bounds = Bounds::new(point(px(2.0), px(3.0)), size(px(0.125), px(1.0)));
        assert_eq!(
            wheel_position(bounds, [0.0; 4], bounds, false, 1.0),
            Some(bounds.center())
        );
        assert_eq!(
            wheel_position(bounds, [0.0; 4], Bounds::default(), false, 1.0),
            None
        );
        let large = Bounds::new(point(px(-10.0), px(-10.0)), size(px(100.0), px(100.0)));
        let clip = Bounds::new(point(px(5.0), px(6.0)), size(px(20.0), px(10.0)));
        assert_eq!(
            wheel_position(large, [0.0; 4], clip, false, 1.0),
            Some(point(px(24.5), px(11.0)))
        );
    }
}
