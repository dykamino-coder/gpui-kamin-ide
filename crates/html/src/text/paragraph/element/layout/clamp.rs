//! Точные строки clamp публикуются при измерении, до prepaint и paint.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Window, px};

impl Paragraph {
    fn measure_clamp_steps(&mut self, limit: Option<Pixels>, window: &mut Window) {
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
        if self.clamp_rows.is_none()
            || self.clamp_tag.is_none()
            || self.vertical
            || self.lines_reversed
        {
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

impl Paragraph {
    fn publish_rows(&self, bounds: Bounds<Pixels>, count: usize, step: &dyn Fn(usize) -> Pixels) {
        if let Some(measure) = &self.clamp_rows
            && let Some(tag) = self.clamp_tag
            && !self.lines_reversed
        {
            let mut y0 = f32::from(bounds.origin.y);
            let rows = match &self.unbalanced_steps {
                Some(steps) => steps
                    .iter()
                    .map(|h| {
                        let r = (y0, y0 + h);
                        y0 = r.1;
                        r
                    })
                    .collect(),
                None => (0..count)
                    .map(|i| {
                        let r = (y0, y0 + f32::from(step(i)));
                        y0 = r.1;
                        r
                    })
                    .collect(),
            };
            measure.publish_rows(tag, rows);
        }
    }
}
