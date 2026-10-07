//! Convert screen pointer coordinates into the same flat space used to paint native vertical text.

use super::*;

impl Paragraph {
    pub(super) fn index_at(
        &self,
        segs: &[Seg],
        bounds: Bounds<Pixels>,
        at: Point<Pixels>,
    ) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let at = match self.selection_vertical {
            Some((physical, ccw)) => unrotate_pointer(physical, ccw, at),
            None => at,
        };
        let visual = ((at.y - bounds.origin.y) / self.line_height)
            .floor()
            .max(0.0) as usize;
        let visual = visual.min(self.lines.len() - 1);
        let row = if self.selection_vertical.is_some() && self.lines_reversed {
            self.lines.len() - 1 - visual
        } else {
            visual
        };
        let line = &self.lines[row];
        let want = at.x - bounds.origin.x + self.x_at(segs, line.range.start, Edge::Start);
        let mut best = line.range.start;
        for (i, _) in self.text[line.range.clone()].char_indices() {
            let idx = line.range.start + i;
            if self.x_at(segs, idx, Edge::End) > want {
                break;
            }
            best = idx;
        }
        best
    }
}

fn unrotate_pointer(bounds: Bounds<Pixels>, ccw: bool, at: Point<Pixels>) -> Point<Pixels> {
    if ccw {
        point(
            bounds.origin.x + bounds.origin.y + bounds.size.height - at.y,
            bounds.origin.y + at.x - bounds.origin.x,
        )
    } else {
        point(
            bounds.origin.x + at.y - bounds.origin.y,
            bounds.origin.y + bounds.origin.x + bounds.size.width - at.x,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_inverse_matches_actual_gpui_paint_matrix_at_fractional_scales() {
        let bounds = Bounds {
            origin: point(px(17.0), px(29.0)),
            size: size(px(30.0), px(80.0)),
        };
        for ccw in [false, true] {
            let paragraph = Paragraph::empty().vertical_counter_clockwise(ccw);
            for scale in [1.0, 1.25, 2.0] {
                let matrix = paragraph.vertical_transform(bounds, scale);
                for local in [(3.0, 5.0), (40.0, 15.0), (75.0, 25.0)] {
                    let flat = bounds.origin + point(px(local.0), px(local.1));
                    let screen = matrix.apply(point(flat.x * scale, flat.y * scale));
                    let restored =
                        unrotate_pointer(bounds, ccw, point(screen.x / scale, screen.y / scale));
                    assert!((f32::from(restored.x - flat.x)).abs() < 0.0001);
                    assert!((f32::from(restored.y - flat.y)).abs() < 0.0001);
                }
            }
        }
    }

    #[test]
    fn native_vertical_lr_pointer_uses_reversed_content_order() {
        let physical = Bounds {
            origin: point(px(17.0), px(29.0)),
            size: size(px(30.0), px(80.0)),
        };
        let flat = Bounds {
            origin: physical.origin,
            size: size(physical.size.height, physical.size.width),
        };
        let mut paragraph = Paragraph::new(
            "abc".into(),
            Vec::new(),
            px(10.0),
            px(10.0),
            Align::Left,
            Wrap::default(),
        );
        paragraph.lines = (0..3)
            .map(|index| Line {
                range: index..index + 1,
                width: px(10.0),
                ellipsis: false,
                clamped: false,
                hyphen: false,
                indent: px(0.0),
            })
            .collect();
        paragraph.selection_vertical = Some((physical, false));
        paragraph.lines_reversed = true;
        let matrix = paragraph.vertical_transform(physical, 1.0);
        for (y, expected) in [(5.0, 2), (15.0, 1), (25.0, 0)] {
            let screen = matrix.apply(flat.origin + point(px(3.0), px(y)));
            assert_eq!(paragraph.index_at(&[], flat, screen), expected);
        }
    }

    #[test]
    fn pointer_inverse_uses_rounded_content_extent_in_a_stretched_left_flow() {
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
        let bounds = Bounds {
            origin: point(px(0.0), px(29.0)),
            size: size(px(100.0), px(80.0)),
        };
        let painted = paragraph.vertical_paint_bounds(bounds, point(px(0.3), px(29.0)), 1.25);
        assert_eq!(painted.size.width, px(18.4));
        let flat = painted.origin + point(px(3.0), px(5.0));
        let matrix = paragraph.vertical_transform(painted, 1.25);
        let screen = matrix.apply(point(flat.x * 1.25, flat.y * 1.25));
        let restored = unrotate_pointer(painted, false, point(screen.x / 1.25, screen.y / 1.25));
        assert!((f32::from(restored.x - flat.x)).abs() < 0.0001);
        assert!((f32::from(restored.y - flat.y)).abs() < 0.0001);
    }
}
