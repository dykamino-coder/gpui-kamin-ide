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
    pub(super) animation: crate::interactive::scroll_animation::Scroll,
}

impl RootBars {
    /// A diagnostic wheel target: the root's vertical bar lane, whose hit
    /// area is the root's own, so the wheel scrolls the root whatever the
    /// content beside the bar holds (a nested scroller), as a browser
    /// scrolls the box whose scrollbar the wheel is over. Without a lane
    /// (overlay or no bar), the visible content beside where it would be.
    #[cfg(feature = "engine-bench")]
    pub(crate) fn wheel_position(&self, window: &Window) -> Option<gpui::Point<Pixels>> {
        let edges = self.style.borders();
        let edge = |v| match v {
            Some(crate::style::values::value::Len::Px(v)) => v,
            _ => 0.0,
        };
        let borders = [
            edge(edges.left),
            edge(edges.top),
            edge(edges.right),
            edge(edges.bottom),
        ];
        wheel_position(
            self.bounds,
            borders,
            self.extent.insets(),
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
        let alpha =
            self.interaction
                .paint(self.freeze, hitbars, &self.handle, &self.animation, window);
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

/// The centre of the vertical gutter on the bar's side (`left`) inside the
/// borders, its height clipped to the native listener `clip` (which can
/// include replay overscan); without a gutter, the visible content box
/// beside it, excluding borders and gutters.
#[cfg(any(feature = "engine-bench", test))]
fn wheel_position(
    bounds: Bounds<Pixels>,
    borders: [f32; 4],
    gutter: [f32; 4],
    clip: Bounds<Pixels>,
    left: bool,
    scale: f32,
) -> Option<gpui::Point<Pixels>> {
    use gpui::{point, px};
    let lane = if left { gutter[0] } else { gutter[2] };
    if lane > 0.0 {
        let x = if left {
            bounds.left() + px(borders[0] + lane / 2.0)
        } else {
            bounds.right() - px(borders[2] + lane / 2.0)
        };
        let top = (bounds.top() + px(borders[1] + gutter[1])).max(clip.top());
        let bottom = (bounds.bottom() - px(borders[3] + gutter[3])).min(clip.bottom());
        if bottom > top && x >= clip.left() && x < clip.right() {
            return Some(point(x, (top + bottom) / 2.0));
        }
    }
    let insets: [f32; 4] = std::array::from_fn(|i| borders[i] + gutter[i]);
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
    fn wheel_target_is_the_centre_of_the_vertical_lane() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0)));
        let native = Bounds::new(
            point(px(-4000.0), px(-4000.0)),
            size(px(8785.0), px(8600.0)),
        );
        // A horizontal bar's gutter shortens the lane; borders move it in.
        assert_eq!(
            wheel_position(bounds, [0.0; 4], [0.0, 0.0, 15.0, 15.0], native, false, 1.0),
            Some(point(px(792.5), px(292.5)))
        );
        assert_eq!(
            wheel_position(
                bounds,
                [2.0, 0.0, 0.0, 0.0],
                [15.0, 0.0, 0.0, 0.0],
                native,
                true,
                2.0
            ),
            Some(point(px(9.5), px(300.0)))
        );
    }

    #[test]
    fn without_a_lane_the_target_is_the_content_beside_the_bar() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(800.0), px(600.0)));
        // A lane outside the listener is no target either.
        let clip = Bounds::new(point(px(0.0), px(0.0)), size(px(785.0), px(600.0)));
        assert_eq!(
            wheel_position(bounds, [0.0; 4], [0.0, 0.0, 15.0, 0.0], clip, false, 1.0),
            Some(point(px(784.5), px(300.0)))
        );
        assert_eq!(
            wheel_position(bounds, [0.0; 4], [0.0; 4], bounds, false, 2.0),
            Some(point(px(799.75), px(300.0)))
        );
    }

    #[test]
    fn clipped_narrow_and_empty_scrollports_keep_the_target_inside() {
        let bounds = Bounds::new(point(px(2.0), px(3.0)), size(px(0.125), px(1.0)));
        assert_eq!(
            wheel_position(bounds, [0.0; 4], [0.0; 4], bounds, false, 1.0),
            Some(bounds.center())
        );
        assert_eq!(
            wheel_position(bounds, [0.0; 4], [0.0; 4], Bounds::default(), false, 1.0),
            None
        );
        let large = Bounds::new(point(px(-10.0), px(-10.0)), size(px(100.0), px(100.0)));
        let clip = Bounds::new(point(px(5.0), px(6.0)), size(px(20.0), px(10.0)));
        assert_eq!(
            wheel_position(large, [0.0; 4], [0.0; 4], clip, false, 1.0),
            Some(point(px(24.5), px(11.0)))
        );
    }
}
