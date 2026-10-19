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
        start: usize,
        end: usize,
        edges: [f32; 4],
        bounds: Bounds<Pixels>,
    ) -> Option<Bounds<Pixels>> {
        if self.lines_reversed || self.lines.is_empty() {
            return None;
        }
        let end = end.min(self.text.len());
        let start = start.min(end);
        let rows: Vec<usize> = (0..self.lines.len())
            .filter(|&i| {
                let r = &self.lines[i].range;
                r.start < end.max(start + 1) && start < r.end
            })
            .collect();
        if rows.len() <= 1 {
            return self.inline_cb_rect(segs, start, end, edges, bounds);
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
            if let Some(f) = self.inline_cb_rect(segs, a, b, e, bounds) {
                out = Some(out.map_or(f, |u| u.union(&f)));
            }
        }
        out
    }
}
