//! Bounded line geometry for the subscribed DevTools journal, using the
//! metrics and breaks of the paragraph being painted.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels};
use serde_json::{Value, json};

impl Paragraph {
    pub(super) fn journal_geometry(&self, bounds: Bounds<Pixels>, window: &mut gpui::Window) {
        crate::why::note("text.line_geometry", || {
            let metrics = self.measure_runs(window);
            self.geometry_snapshot(bounds, &metrics).to_string()
        });
    }

    fn geometry_snapshot(&self, bounds: Bounds<Pixels>, metrics: &[(f32, f32)]) -> Value {
        let pads = self.line_padding();
        let mut y = f32::from(bounds.origin.y);
        let lines: Vec<_> = self
            .lines
            .iter()
            .take(30)
            .enumerate()
            .map(|(index, line)| {
                let (above, below) = pads.get(index).copied().unwrap_or_default();
                let top = y + above;
                let height = f32::from(self.line_height) + above + below;
                let result = json!({
                    "start": line.range.start, "end": line.range.end,
                    "text": &self.text[line.range.clone()],
                    "top": top, "height": height,
                    "baseline": top + self.base_with_metrics(&line.range, metrics)
                        .unwrap_or_else(|| self.line_base(&line.range)),
                    "width": f32::from(line.width), "indent": f32::from(line.indent),
                });
                y += height;
                result
            })
            .collect();
        let fonts: Vec<_> = self
            .runs
            .iter()
            .enumerate()
            .take(30)
            .map(|(index, run)| {
                json!({"family": run.font.family.as_ref(), "weight": run.font.weight.0,
                "style": format!("{:?}", run.font.style),
                "size": f32::from(run.font_size.unwrap_or(self.font_size)),
                "extents": metrics.get(index)})
            })
            .collect();
        json!({"origin": [f32::from(bounds.origin.x), f32::from(bounds.origin.y)],
            "size": [f32::from(bounds.size.width), f32::from(bounds.size.height)],
            "line_height": f32::from(self.line_height), "text_bytes": self.text.len(),
            "strut": self.strut,
            "vertical": self.vertical, "reversed": self.lines_reversed,
            "fonts": fonts, "lines": lines})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    #[test]
    fn snapshot_bounds_lines_and_preserves_fractional_geometry() {
        let mut paragraph = Paragraph::new(
            "x".repeat(40).into(),
            Vec::new(),
            px(16.0),
            px(18.375),
            Align::Left,
            Wrap::default(),
        );
        paragraph.lines = (0..40).map(|i| Line::test(i..i + 1, px(12.25))).collect();
        let snapshot = paragraph.geometry_snapshot(
            Bounds {
                origin: point(px(2.5), px(3.25)),
                size: size(px(100.0), px(735.0)),
            },
            &[],
        );
        assert_eq!(snapshot["lines"].as_array().unwrap().len(), 30);
        assert_eq!(snapshot["lines"][0]["top"], 3.25);
        assert_eq!(snapshot["lines"][1]["top"], 21.625);
        assert_eq!(snapshot["lines"][0]["width"], 12.25);
        assert_eq!(snapshot["lines"][29]["end"], 30);
    }
}
