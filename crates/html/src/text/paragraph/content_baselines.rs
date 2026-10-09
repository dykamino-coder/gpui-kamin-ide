//! Expose the actual line positions used for painting, including intermediate fragment lines.

use super::*;

impl Paragraph {

    pub(super) fn measured_first_baseline(&self, line_height: Pixels, first_above: Pixels, window: &mut Window) -> Option<Pixels> {
        // Use actual post-fit font metrics; descent has a negative native sign.
        let baseline = self.runs.first().map(|run| {
            let font = run.font.clone();
            let size = run.font_size.unwrap_or(self.font_size);
            let id = window.text_system().resolve_font(&font);
            // Полулидинг считается сам: готовая `baseline_offset`
            // ВЫЧИТАЕТ спуск, а он в метриках хранится со знаком
            // минус (`direct_write.rs`), и лишний спуск уходил в
            // отступ сверху — базовая линия вставала ниже верной
            // (Ahem 16px: 14.4 вместо 12.8).
            let ascent = window.text_system().ascent(id, size);
            let descent = window.text_system().descent(id, size);
            let content = ascent + descent.abs();
            (line_height - content) / 2.0 + ascent
        });
        match self.lines.first() {
            Some(first) if !self.atom_boxes.is_empty() || !self.box_spans.is_empty() => {
                Some(px(self.line_base(&first.range)) + first_above)
            }
            _ => baseline,
        }
    }

    pub(super) fn content_line_baselines(&self, first: Option<Pixels>) -> Vec<Pixels> {
        let Some(first) = first else {
            return Vec::new();
        };
        let pads = self.line_padding();
        let atomic = !self.atom_boxes.is_empty() || !self.box_spans.is_empty();
        let mut y = 0.0;
        self.lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let (above, below) = pads.get(index).copied().unwrap_or((0.0, 0.0));
                let base = if atomic {
                    self.line_base(&line.range)
                } else {
                    f32::from(first)
                };
                let baseline = px(y + above + base);
                y += f32::from(self.line_height) + above + below;
                baseline
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taller_middle_line_has_its_own_baseline_and_advances_later_lines() {
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
        paragraph.lh_spans = vec![(1..2, px(20.0))];
        assert_eq!(
            paragraph.content_line_baselines(Some(px(5.0))),
            vec![px(5.0), px(20.0), px(35.0), px(45.0)]
        );
    }
}
