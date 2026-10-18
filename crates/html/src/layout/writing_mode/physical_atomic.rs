//! Measure atomic descendants in physical coordinates and cancel only the enclosing text turn.
//! Independent native snapshots avoid nested access to the window's borrowed layout engine.

use crate::hooks::Wrap;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, LayoutId, MeasuredContent, Pixels, Style,
    TransformationMatrix, Window, point, px, size,
};

pub(super) struct PhysicalAtomic {
    child: AnyElement,
    root: Option<LayoutId>,
    ccw: bool,
    central: bool,
    inverse_text: TransformationMatrix,
}

impl PhysicalAtomic {
    pub(super) fn new(child: AnyElement, ccw: bool, central: bool) -> Self {
        Self {
            child,
            root: None,
            ccw,
            central,
            inverse_text: TransformationMatrix::unit(),
        }
    }
}

fn flat_baseline(width: Pixels, baseline: Option<Pixels>, ccw: bool, central: bool) -> Pixels {
    let physical = baseline.unwrap_or(if central { width / 2.0 } else { px(0.0) });
    if ccw { physical } else { width - physical }
}

impl Wrap for PhysicalAtomic {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let root = self.child.request_layout(window, cx);
        self.root = Some(root);
        let mut snapshot = window.snapshot_layout_measurement(root);
        let (ccw, central) = (self.ccw, self.central);
        window.request_measured_layout_with_physical_baselines(
            Style::default(),
            move |known, available, window, cx| {
                let physical = snapshot.measure_physical_baselines(
                    size(
                        known
                            .height
                            .map_or(available.height, AvailableSpace::Definite),
                        known
                            .width
                            .map_or(available.width, AvailableSpace::Definite),
                    ),
                    window,
                    cx,
                );
                MeasuredContent {
                    first_y: Some(flat_baseline(
                        physical.size.width,
                        physical.first_x,
                        ccw,
                        central,
                    )),
                    last_y: Some(flat_baseline(
                        physical.size.width,
                        physical.last_x.or(physical.first_x),
                        ccw,
                        central,
                    )),
                    ..MeasuredContent::new(size(physical.size.height, physical.size.width))
                }
            },
        )
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let root = self.root.expect("physical child requested layout");
        window.compute_layout(
            root,
            size(
                AvailableSpace::Definite(bounds.size.height),
                AvailableSpace::Definite(bounds.size.width),
            ),
            cx,
        );
        let (physical, inverse_text) =
            crate::layout::writing_mode::physical_atomic_frame::map(bounds, window.scale_factor());
        self.inverse_text = inverse_text;
        window.set_layout_root_origin(root, physical.origin);
        crate::layout::writing_mode::physical_atomic_frame::without_frame(|| {
            self.child.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        });
    }

    fn paint(&mut self, _bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.with_transformation_masked(self.inverse_text, |window| self.child.paint(window, cx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabetic_and_synthesized_baselines_follow_physical_edges() {
        assert_eq!(
            flat_baseline(px(100.0), Some(px(23.0)), false, false),
            px(77.0)
        );
        assert_eq!(
            flat_baseline(px(100.0), Some(px(23.0)), true, false),
            px(23.0)
        );
        assert_eq!(flat_baseline(px(100.0), None, false, true), px(50.0));
        assert_eq!(flat_baseline(px(100.0), None, false, false), px(100.0));
    }
}
