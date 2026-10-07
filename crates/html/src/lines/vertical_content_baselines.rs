//! Export vertical paragraph baselines from the same line origins used for painting.
//! Right-origin offsets are translated by native layout after the final width is known.

use super::*;

impl Paragraph {
    /// Select the dominant vertical baseline: central for upright/mixed, alphabetic for sideways.
    pub fn vertical_central_baseline(mut self, central: bool) -> Self {
        self.vertical_central_baseline = central;
        self
    }

    /// The paragraph is a rotated vertical line set with a central dominant baseline.
    pub fn rotated_central(mut self, on: bool) -> Self {
        self.rotated_central = on;
        self
    }

    pub(super) fn vertical_content_baselines(
        &self,
        content_size: gpui::Size<Pixels>,
    ) -> gpui::MeasuredContent {
        let pads = self.line_padding();
        let step = |index: usize| {
            let (above, below) = pads.get(index).copied().unwrap_or((0.0, 0.0));
            f32::from(self.line_height) + above + below
        };
        // Clockwise paint starts at the right edge; sideways-lr starts at the left.
        let mut origin = if self.lines_reversed && !self.lines.is_empty() {
            f32::from(self.vertical_line_extent() - self.line_height)
        } else {
            0.0
        };
        if self.lines_reversed && !self.vertical_ccw {
            origin += f32::from(content_size.width - self.vertical_line_extent());
        }
        let lines: Vec<_> = self
            .lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let above = pads.get(index).map_or(0.0, |pad| pad.0);
                let base = if self.vertical_central_baseline {
                    f32::from(self.line_height) / 2.0
                } else {
                    self.line_base(&line.range)
                };
                let offset = px(origin + above + base);
                origin += if self.lines_reversed {
                    -step(index)
                } else {
                    step(index)
                };
                offset
            })
            .collect();
        gpui::MeasuredContent {
            first_x: lines.first().copied(),
            last_x: lines.last().copied(),
            lines_x: Some(lines),
            x_from_right: !self.vertical_ccw,
            ..gpui::MeasuredContent::new(content_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph() -> Paragraph {
        let mut paragraph = Paragraph::new(
            "abcd".into(),
            Vec::new(),
            px(10.0),
            px(10.0),
            Align::Left,
            Wrap::default(),
        );
        paragraph.lines = (0..4)
            .map(|index| Line {
                range: index..index + 1,
                width: px(10.0),
                ellipsis: false,
                clamped: false,
                vis_cut: None,
                hyphen: false,
                indent: px(0.0),
            })
            .collect();
        paragraph.strut = (8.0, 2.0, 4.0);
        paragraph
    }

    #[test]
    fn central_offsets_follow_actual_variable_line_advances() {
        let mut paragraph = paragraph();
        paragraph.lh_spans = vec![(1..2, px(20.0))];
        let output = paragraph.vertical_content_baselines(size(px(50.0), px(10.0)));
        assert_eq!(
            output.lines_x,
            Some(vec![px(5.0), px(20.0), px(35.0), px(45.0)])
        );
        assert_eq!(output.first_x, Some(px(5.0)));
        assert_eq!(output.last_x, Some(px(45.0)));
        assert!(output.x_from_right);
        assert_eq!(output.first_y, None);
        assert_eq!(output.last_y, None);
    }

    #[test]
    fn reversed_content_order_is_preserved_without_sorting_coordinates() {
        let paragraph = paragraph().reversed_lines(true);
        let output = paragraph.vertical_content_baselines(size(px(40.0), px(10.0)));
        assert_eq!(
            output.lines_x,
            Some(vec![px(35.0), px(25.0), px(15.0), px(5.0)])
        );
        assert_eq!(output.first_x, Some(px(35.0)));
        assert_eq!(output.last_x, Some(px(5.0)));
    }

    #[test]
    fn left_flow_baselines_match_content_positions_inside_extra_physical_width() {
        let paragraph = paragraph().reversed_lines(true);
        let output = paragraph.vertical_content_baselines(size(px(100.0), px(10.0)));
        let offsets = output.lines_x.unwrap();
        assert!(output.x_from_right);
        let physical: Vec<_> = offsets
            .into_iter()
            .map(|offset| px(100.0) - offset)
            .collect();
        assert_eq!(physical, vec![px(5.0), px(15.0), px(25.0), px(35.0)]);
    }

    #[test]
    fn sideways_uses_real_alphabetic_metrics_instead_of_the_central_line() {
        let paragraph = paragraph().vertical_central_baseline(false);
        let output = paragraph.vertical_content_baselines(size(px(40.0), px(10.0)));
        assert_eq!(
            output.lines_x,
            Some(vec![px(8.0), px(18.0), px(28.0), px(38.0)])
        );
    }

    #[test]
    fn sideways_lr_alphabetic_lines_advance_from_the_left_edge() {
        let paragraph = paragraph()
            .vertical_central_baseline(false)
            .vertical_counter_clockwise(true);
        let output = paragraph.vertical_content_baselines(size(px(40.0), px(10.0)));
        assert_eq!(
            output.lines_x,
            Some(vec![px(8.0), px(18.0), px(28.0), px(38.0)])
        );
        assert!(!output.x_from_right);
        assert_eq!(output.first_x, Some(px(8.0)));
        assert_eq!(output.last_x, Some(px(38.0)));
    }
}
