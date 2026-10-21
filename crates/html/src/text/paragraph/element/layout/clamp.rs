//! Точные строки clamp публикуются при измерении, до prepaint и paint.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Window, px};

impl Paragraph {
    pub(crate) fn measure_clamp_steps(&mut self, limit: Option<Pixels>, window: &mut Window) {
        self.unbalanced_steps = None;
        if self.clamp_tag.is_some() && self.wrap.balance && self.clamp.is_none() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            let segs = self.measure(window);
            let unbalanced = self.lay(limit, &segs);
            let balanced = std::mem::replace(&mut self.lines, unbalanced);
            let lh = f32::from(self.line_height);
            self.unbalanced_steps = Some(
                self.line_padding()
                    .iter()
                    .map(|(a, b)| lh + a + b)
                    .collect(),
            );
            self.lines = balanced;
        }
    }

    pub(super) fn publish_measured_clamp(
        &mut self,
        limit: Option<Pixels>,
        lh: Pixels,
        window: &mut Window,
    ) {
        if self.clamp_tag.is_none() || self.vertical || self.lines_reversed {
            return;
        }
        self.measure_clamp_steps(limit, window);
        let pads = self.line_padding();
        let step = |i: usize| {
            let (a, b) = pads.get(i).copied().unwrap_or_default();
            lh + px(a + b)
        };
        self.publish_rows(Bounds::default(), self.lines.len(), &step);
    }
}
