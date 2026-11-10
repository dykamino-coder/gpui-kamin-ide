//! CSSOM-фрагменты читаются из строк, метрик и видимого порядка раскладки.
//! Якорный containing block имеет иные правила (включая static position),
//! поэтому запрос не использует его прямоугольник вместо border fragments.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, point, px, size};

impl Paragraph {
    pub(crate) fn visit_inline_fragments(
        &self,
        segs: &[Seg],
        range: std::ops::Range<usize>,
        edges: [f32; 4],
        metrics: (f32, f32),
        bounds: Bounds<Pixels>,
        mut emit: impl FnMut(Bounds<Pixels>, [bool; 4], (Pixels, Pixels)),
    ) {
        let end = range.end.min(self.text.len());
        let start = range.start.min(end);
        let pads = self.line_padding();
        if self.lines.is_empty() && start == end {
            let at = self.point_of(segs, start, bounds);
            emit(
                Bounds::new(
                    point(at.x - px(edges[3]), at.y - px(edges[0])),
                    size(
                        px(edges[1] + edges[3]),
                        px(metrics.0 + metrics.1 + edges[0] + edges[2]),
                    ),
                ),
                [true; 4],
                (bounds.origin.y, px(0.)),
            );
            return;
        }
        let step = |i: usize| {
            let (a, b) = pads.get(i).copied().unwrap_or_default();
            self.line_height + px(a + b)
        };
        let mut y = bounds.origin.y + px(self.reversed_offset(self.lines.len(), &step));
        for (row, line) in self.lines.iter().enumerate() {
            let last = row + 1 == self.lines.len();
            if start < line.range.end && end > line.range.start
                || start == end && start >= line.range.start && (start < line.range.end || last)
            {
                let text_end =
                    line.range.start + self.text[line.range.clone()].trim_end_matches('\n').len();
                let visible_end = if self.wrap.keep_spaces {
                    text_end
                } else {
                    self.drop_collapsible_tail(line.range.start, text_end)
                };
                let visible = line.range.start..visible_end;
                let a = start.clamp(visible.start, visible.end);
                let b = end.clamp(a, visible.end);
                let align = self.line_align(row, line);
                let free = bounds.size.width + self.width_nudge
                    - line.width
                    - line.indent
                    - px(self.flow_cut(row).1);
                let hang = self.hang_first(line.range.start);
                let lead = if self.wrap.rtl { px(0.) } else { line.indent }
                    - self.span(segs, line.range.start, line.range.start + hang)
                    + px(self.flow_cut(row).0);
                let left = bounds.origin.x + lead + line_offset(align, self.wrap.rtl, free);
                let (_, _, stretch, absorbed, from) = self.prepare_words(
                    &visible,
                    segs,
                    if align == Align::Justify {
                        free.max(px(0.))
                    } else {
                        px(0.)
                    },
                );
                let advance = |pos: usize| {
                    self.x_at(segs, pos, Edge::Start) - from
                        + stretch * self.justify_opps(&visible, pos).saturating_sub(absorbed) as f32
                };
                let visual = self
                    .resolved_visual(&visible, true)
                    .map(|(_, runs)| runs)
                    .unwrap_or_else(|| vec![(visible.start, visible.end, self.wrap.rtl)]);
                let mut placed = self.place_visual(&visual, &advance);
                if placed.is_empty() && a == b {
                    placed.push((a, b, self.wrap.rtl, px(0.)));
                }
                let baseline = y + px(
                    pads.get(row).copied().unwrap_or_default().0 + self.line_base(&line.range)
                );
                let mut fragments: Vec<(usize, usize, bool, Pixels, Pixels)> = Vec::new();
                for (s, e, rtl, offset) in placed {
                    let lo = a.max(s);
                    let hi = b.min(e);
                    if lo > hi || lo == hi && (a != b || a == e && e != visible.end) {
                        continue;
                    }
                    let width = (advance(hi) - advance(lo)).max(px(0.));
                    let x = left
                        + offset
                        + if rtl {
                            advance(e) - advance(hi)
                        } else {
                            advance(lo) - advance(s)
                        };
                    // Границы вложенных коробок режут placement, но сами по
                    // себе не создают новый фрагмент внешнего элемента.
                    if let Some(prev) = fragments.last_mut()
                        && prev.2 == rtl
                        && prev.3 + prev.4 == x
                        && (prev.1 == lo || hi == prev.0)
                    {
                        prev.0 = prev.0.min(lo);
                        prev.1 = prev.1.max(hi);
                        prev.4 += width;
                    } else {
                        fragments.push((lo, hi, rtl, x, width));
                    }
                }
                // CSSOM View требует порядок содержимого, а не порядок краски bidi.
                fragments.sort_by_key(|fragment| fragment.0);
                for (lo, hi, rtl, x, width) in fragments {
                    let mut present = [true, false, true, false];
                    let (leading, trailing) = if rtl { (1, 3) } else { (3, 1) };
                    present[leading] = lo == start;
                    present[trailing] = hi == end;
                    let edge = |i| if present[i] { edges[i] } else { 0. };
                    emit(
                        Bounds::new(
                            point(x - px(edge(3)), baseline - px(metrics.0 + edge(0))),
                            size(
                                width + px(edge(1) + edge(3)),
                                px(metrics.0 + metrics.1 + edge(0) + edge(2)),
                            ),
                        ),
                        present,
                        (y, step(row)),
                    );
                }
            }
            if self.lines_reversed {
                y -= step(row + 1);
            } else {
                y += step(row);
            }
        }
    }
}
