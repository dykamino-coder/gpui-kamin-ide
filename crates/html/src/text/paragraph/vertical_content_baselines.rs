//! Базовые линии вертикального абзаца для родителя — от тех же начал строк,
//! что и отрисовка: центральная доминантная базовая для upright/mixed,
//! алфавитная для sideways; смещения от правого края раскладка переводит
//! сама, когда известна окончательная ширина. Здесь же выбор доминантной
//! базовой и признак повёрнутой строки с центральной базовой.

use super::*;
use gpui::{Pixels, px};

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
            self.line_height + px(above + below)
        };
        // Clockwise paint starts at the right edge; sideways-lr starts at the left.
        let mut origin = self.reversed_offset(self.lines.len(), &step);
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
                    -f32::from(step(index + 1))
                } else {
                    f32::from(step(index))
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
    use gpui::size;

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
            .map(|index| Line::test(index..index + 1, px(10.0)))
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
    fn reversed_lines_keep_each_annotation_outset_in_its_own_advance() {
        let mut paragraph = paragraph().reversed_lines(true);
        paragraph.lh_spans = vec![(0..1, px(20.0)), (2..3, px(30.0))];
        let output = paragraph.vertical_content_baselines(size(px(70.0), px(10.0)));
        assert_eq!(
            output.lines_x,
            Some(vec![px(60.0), px(45.0), px(25.0), px(5.0)])
        );
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

    /// `font: 60px/1.5 Ahem` turned with a central baseline and runs of 120px
    /// and 30px (`text-baseline-vrl-002`): the line height is the root inline
    /// box's computed 90px, not 1.5 times the largest run.
    fn turned_central_line(rotated: bool) -> Paragraph {
        let runs = (0..3)
            .map(|_| gpui::TextRun {
                len: 1,
                ..Default::default()
            })
            .collect();
        let mut paragraph = Paragraph::new(
            "ABO".into(),
            runs,
            px(60.0),
            px(90.0),
            Align::Left,
            Wrap::default(),
        )
        .rotated_central(rotated);
        paragraph.lines = vec![Line::test(0..3, px(150.0))];
        paragraph.run_metrics = vec![(48.0, 12.0), (96.0, 24.0), (24.0, 6.0)];
        paragraph.strut_box = (48.0, 12.0);
        paragraph
    }

    #[test]
    fn turned_central_runs_share_the_root_inline_baseline() {
        let paragraph = turned_central_line(true);
        // Half-leading 15 plus the strut ascent 48; the 120px run would put
        // the whole-line baseline at 81 and every glyph 18px off centre.
        assert_eq!(paragraph.shared_base(&(0..3)), Some(63.0));
        assert_eq!(paragraph.line_base(&(0..3)), 63.0);
        // The root box's central baseline is the line's reported centre.
        let central = 63.0 - (48.0 - 12.0) / 2.0;
        let output = paragraph.vertical_content_baselines(size(px(90.0), px(150.0)));
        assert_eq!(output.first_x, Some(px(central)));
    }

    #[test]
    fn horizontal_runs_keep_the_whole_line_shaping_baseline() {
        let paragraph = turned_central_line(false);
        assert_eq!(paragraph.shared_base(&(0..3)), Some(81.0));
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
