//! Paint native vertical text about the matching physical edge for both sideways directions.

use super::*;

impl Paragraph {
    pub(super) fn vertical_line_extent(&self) -> Pixels {
        let extra: f32 = self
            .line_padding()
            .iter()
            .map(|(above, below)| above + below)
            .sum();
        self.line_height * self.lines.len() as f32 + px(extra)
    }

    pub(super) fn vertical_paint_bounds(
        &self,
        mut bounds: Bounds<Pixels>,
        layout_origin: Point<Pixels>,
        scale: f32,
    ) -> Bounds<Pixels> {
        // Clockwise vertical-lr reverses the flat lines to start at the left.
        // Anchor that content extent at the left even when layout stretches
        // the paragraph's physical width beyond its painted columns.
        if self.lines_reversed && !self.vertical_ccw {
            // Match GPUI's rounded absolute layout edges, including a
            // fractional origin; rounding the extent alone loses that phase.
            let start = f32::from(layout_origin.x) * scale;
            let end = start + f32::from(self.vertical_line_extent()) * scale;
            bounds.size.width = px((end.round() - start.round()) / scale);
        }
        bounds
    }

    pub fn vertical_counter_clockwise(mut self, counter_clockwise: bool) -> Self {
        self.vertical_ccw = counter_clockwise;
        self
    }

    pub(super) fn vertical_transform(
        &self,
        bounds: Bounds<Pixels>,
        scale: f32,
    ) -> gpui::TransformationMatrix {
        let (corner, angle) = if self.vertical_ccw {
            (
                point(bounds.origin.x, bounds.origin.y + bounds.size.height),
                -std::f32::consts::FRAC_PI_2,
            )
        } else {
            (
                point(bounds.origin.x + bounds.size.width, bounds.origin.y),
                std::f32::consts::FRAC_PI_2,
            )
        };
        let dev = |value: Pixels| value.scale(scale);
        gpui::TransformationMatrix::unit()
            .translate(point(dev(corner.x), dev(corner.y)))
            .rotate(gpui::Radians(angle))
            .translate(point(dev(-bounds.origin.x), dev(-bounds.origin.y)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_left_flow_anchors_content_in_a_stretched_physical_box() {
        let mut paragraph = Paragraph::empty().reversed_lines(true);
        paragraph.line_height = px(10.0);
        paragraph.lines = vec![Line {
            range: 0..0,
            width: px(20.0),
            ellipsis: false,
            clamped: false,
            hyphen: false,
            indent: px(0.0),
        }];
        let bounds = Bounds {
            origin: point(px(17.0), px(29.0)),
            size: size(px(100.0), px(80.0)),
        };
        for scale in [1.0, 1.25, 2.0] {
            let painted = paragraph.vertical_paint_bounds(bounds, point(px(0.0), px(0.0)), scale);
            let extent = if scale == 1.25 { 10.4 } else { 10.0 };
            assert_eq!(painted.size.width, px(extent));
            let matrix = paragraph.vertical_transform(painted, scale);
            let flat = bounds.origin + point(px(3.0), px(5.0));
            let screen = matrix.apply(point(flat.x * scale, flat.y * scale));
            assert!((f32::from(screen.x / scale - px(17.0 + extent - 5.0))).abs() < 0.0001);
            assert!((f32::from(screen.y / scale - px(32.0))).abs() < 0.0001);
        }
    }

    #[test]
    fn left_flow_content_uses_absolute_device_edges_at_fractional_origins() {
        let mut paragraph = Paragraph::empty().reversed_lines(true);
        paragraph.line_height = px(17.71875);
        paragraph.lines = vec![Line {
            range: 0..0,
            width: px(20.0),
            ellipsis: false,
            clamped: false,
            hyphen: false,
            indent: px(0.0),
        }];
        // With origin 0.3 at 125%, the edges round to 0 and 23 device
        // pixels. Rounding the width independently would produce 22.
        for (origin, expected) in [(0.0, 17.6), (0.3, 18.4), (0.6, 17.6)] {
            let bounds = Bounds {
                origin: point(px((origin * 1.25_f32).round() / 1.25), px(0.0)),
                size: size(px(100.0), px(80.0)),
            };
            let painted = paragraph.vertical_paint_bounds(bounds, point(px(origin), px(0.0)), 1.25);
            assert_eq!(painted.origin, bounds.origin);
            assert_eq!(painted.size.width, px(expected));
        }
    }
}
