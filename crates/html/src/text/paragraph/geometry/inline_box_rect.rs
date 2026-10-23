//! Рамка строчной коробки из нескольких фрагментов (якорь css-anchor-position-1).

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels};

impl Paragraph {
    /// Объемлющий прямоугольник border box фрагментов строчной коробки с
    /// содержимым `start..end` (css-anchor-position-1 §anchor box: «the
    /// bounding rectangle of all the box fragments»; Blink `AnchorMap::Set`
    /// объединяет фрагменты одного объекта). Каждый фрагмент — отрезок своей
    /// строки (`inline_cb_rect` в пределах строки); `edges` — отбивка с
    /// рамкой `[top, right, bottom, left]`: строчные края — только у первого
    /// и последнего фрагмента (`box-decoration-break: slice`), блочные — у
    /// всех. Пустой отрезок строки фрагмента не даёт.
    pub(crate) fn inline_box_rect(
        &self,
        segs: &[Seg],
        range: std::ops::Range<usize>,
        edges: [f32; 4],
        metrics: (f32, f32),
        bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        if self.lines_reversed || self.lines.is_empty() {
            return None;
        }
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        let rows: Vec<usize> = (0..self.lines.len())
            .filter(|&i| {
                let r = &self.lines[i].range;
                r.start < end.max(start + 1) && start < r.end
            })
            .collect();
        if rows.len() <= 1 {
            return self.inline_fragment_rect(segs, start..end, edges, metrics, bounds);
        }
        let rtl = self.wrap.rtl;
        let blank = |s: &str| s.chars().all(|c| matches!(c, ' ' | '\n' | '\t'));
        let mut out: Option<Bounds<Pixels>> = None;
        let last = rows.len() - 1;
        for (k, &row) in rows.iter().enumerate() {
            let r = &self.lines[row].range;
            let (a, b) = (start.max(r.start), end.min(r.end));
            if b <= a || blank(&self.text[a..b]) {
                continue;
            }
            // Начальный край — у первого фрагмента, конечный — у последнего;
            // при rtl начало строки справа.
            let (first, fin) = (k == 0, k == last);
            let (lead, trail) = if rtl { (1, 3) } else { (3, 1) };
            let mut e = [edges[0], 0.0, edges[2], 0.0];
            if first {
                e[lead] = edges[lead];
            }
            if fin {
                e[trail] = edges[trail];
            }
            if let Some(f) = self.inline_fragment_rect(segs, a..b, e, metrics, bounds) {
                out = Some(out.map_or(f, |u| u.union(&f)));
            }
        }
        out
    }
}

impl Paragraph {
    /// Content area собственного шрифта коробки, без полулидинга строки
    /// (CSS 2 §10.6.1, §10.8.1). Строка и рамка якоря имеют разные края.
    fn inline_fragment_rect(
        &self,
        segs: &[Seg],
        range: std::ops::Range<usize>,
        edges: [f32; 4],
        metrics: (f32, f32),
        bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        let mut rect = self.inline_cb_rect(segs, range.start, range.end, edges, bounds)?;
        let row = self.lines.iter().position(|l| range.start < l.range.end)?;
        let (a, d) = metrics;
        let pads = self.line_padding();
        let before: f32 = pads[..row].iter().map(|(a, b)| a + b).sum();
        let baseline = bounds.origin.y
            + self.line_height * row as f32
            + gpui::px(before + pads[row].0 + self.line_base(&self.lines[row].range));
        rect.origin.y = baseline - gpui::px(a + edges[0]);
        rect.size.height = gpui::px(a + d + edges[0] + edges[2]);
        Some(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TextRun, point, px, size};

    #[test]
    fn anchor_border_box_excludes_half_leading() {
        let mut p = Paragraph::new(
            "x".into(),
            vec![TextRun {
                len: 1,
                ..Default::default()
            }],
            px(20.0),
            px(40.0),
            Align::Left,
            Wrap::default(),
        );
        p.lines = vec![Line::test(0..1, px(20.0))];
        p.run_metrics = vec![(16.0, 4.0)];
        let bounds = Bounds::new(point(px(8.0), px(60.0)), size(px(20.0), px(40.0)));
        let r = p
            .inline_box_rect(&[], 0..1, [2.0; 4], (16.0, 4.0), bounds)
            .unwrap();
        assert_eq!(r.origin.y, px(68.0));
        assert_eq!(r.size.height, px(24.0));
        // Потомок увеличивает строку, но не content area самого якоря.
        p.run_metrics = vec![(24.0, 6.0)];
        let r = p
            .inline_box_rect(&[], 0..1, [2.0; 4], (16.0, 4.0), bounds)
            .unwrap();
        assert_eq!(r.size.height, px(24.0));
        assert_eq!(r.origin.y, px(71.0));
    }
}
