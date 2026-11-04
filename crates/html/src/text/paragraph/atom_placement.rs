//! Расстановка атомов по строкам на prepaint: x — от продвижения до распорки
//! плюс прижим строки по `text-align` (тот же, что у отрисовки), y — от
//! базовой линии строки по `vertical-align`; при `vertical-lr` строки идут
//! снизу вверх. Атом за концом последней строки (обрыв `line-clamp`)
//! прячется. Корень атома ставится на ДРОБНОЕ абсолютное место, и края его
//! коробок округляются от сырого начала абзаца, как в основном дереве.

use super::*;
use gpui::{App, Bounds, LayoutId, Window, point, px};

impl Paragraph {
    pub(super) async fn place_atoms(
        &mut self,
        root: LayoutId,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if self.atoms.is_empty() || self.lines.is_empty() {
            return;
        }
        let bounds = Bounds {
            origin: window.layout_origin_unrounded(root),
            size: window.layout_size_unrounded(root),
        };
        let segs = self.measure(window);
        let pads = self.line_padding();
        let lh = f32::from(self.line_height);
        let mut tops = Vec::with_capacity(self.lines.len());
        let mut y = 0.0f32;
        for (i, _) in self.lines.iter().enumerate() {
            tops.push(y);
            let (p, q) = pads.get(i).copied().unwrap_or((0.0, 0.0));
            y += lh + p + q;
        }
        // `vertical-lr` feeds lines bottom-up (`reversed_offset` in painting):
        // the first line owns the bottom row, so its atoms go there as well.
        if self.lines_reversed {
            let total = y;
            for (i, top) in tops.iter_mut().enumerate() {
                let (p, q) = pads.get(i).copied().unwrap_or((0.0, 0.0));
                *top = total - *top - (lh + p + q);
            }
        }
        let mut cursor = 0;
        let mut row_geometry = None;
        for k in 0..self.atoms.len() {
            let Some(b) = self.atom_boxes.get(k).copied() else {
                continue;
            };
            // Атом за концом последней строки остался в оборванном
            // `line-clamp` хвосте: прежде он вставал на последнюю видимую
            // строку поверх её текста (`line-clamp-auto-with-ruby-002`).
            let Some(row) = atom_row(&self.lines, &mut cursor, b.at) else {
                self.atoms[k].hidden = true;
                continue;
            };
            self.atoms[k].hidden = false;
            let line = self.lines[row].clone();
            let (p, q) = pads.get(row).copied().unwrap_or((0.0, 0.0));
            if row_geometry
                .as_ref()
                .is_some_and(|&(cached, _, _, _)| cached != row)
            {
                row_geometry = None;
            }
            let (_, dx, baseline, start_x) = *row_geometry.get_or_insert_with(|| {
                let align = self.line_align(row, &line);
                let free_raw =
                    bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
                let hang = self.hang_first(line.range.start);
                let shift = self.span(&segs, line.range.start, line.range.start + hang);
                let lead = line.indent - shift;
                let dx = if align == Align::Justify {
                    lead
                } else {
                    line_offset(align, self.wrap.rtl, free_raw) + lead
                };
                (
                    row,
                    dx,
                    self.line_base(&line.range),
                    self.x_at(&segs, line.range.start, Edge::Start),
                )
            });
            let x = dx + self.x_at(&segs, b.at, Edge::Start) - start_x - px(b.shift);
            let top = match b.align {
                AtomAlign::Top => tops[row] + f32::from(line.clearance),
                AtomAlign::Bottom => tops[row] + lh + p + q - b.h,
                _ => tops[row] + p + baseline + self.atom_top(&b),
            };
            // Корень атома ставится на ДРОБНОЕ абсолютное место, и края
            // всех его коробок округляются на абсолютной координате — как в
            // основном дереве (`taffy.rs` `layout_bounds`, KaminIDE patch
            // `set_root_origin`). Округлённый угол корня давал точку
            // расхождения с тем же атомом в гибком ряду
            // (`flexbox-justify-content-horiz-002/004`).
            let origin = point(bounds.origin.x + x, bounds.origin.y + px(top));
            match self.atoms[k].root.get() {
                Some(root) => {
                    window.set_layout_root_origin(root, origin);
                    self.atoms[k]
                        .el
                        .prepaint_at_async(point(px(0.), px(0.)), window, cx, stack)
                        .await;
                }
                None => {
                    self.atoms[k]
                        .el
                        .prepaint_at_async(origin, window, cx, stack)
                        .await;
                }
            }
        }
    }
}

/// Первый конец строки строго после атома. Порядок строк логический даже
/// при vertical-lr; произвольный порядок публичных слотов допускает откат.
fn atom_row(lines: &[Line], cursor: &mut usize, at: usize) -> Option<usize> {
    crate::work::add!(ParagraphLineQueries, 1);
    let mut _steps = 0;
    if *cursor > 0 && lines[*cursor - 1].range.end > at {
        *cursor = lines.partition_point(|line| {
            _steps += 1;
            line.range.end <= at
        });
    } else {
        while *cursor < lines.len() {
            _steps += 1;
            if at < lines[*cursor].range.end {
                break;
            }
            *cursor += 1;
        }
    }
    crate::work::add!(ParagraphLineSteps, _steps);
    (*cursor < lines.len()).then_some(*cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_rows_keep_first_end_semantics_across_gaps_empty_lines_and_clamp() {
        let lines: Vec<_> = [0..0, 0..3, 3..5, 7..10]
            .into_iter()
            .map(|r| Line::test(r, px(8.)))
            .collect();
        let mut cursor = 0;
        for (at, expected) in [
            (0, Some(1)),
            (2, Some(1)),
            (3, Some(2)),
            (5, Some(3)),
            (6, Some(3)),
            (9, Some(3)),
            (10, None),
            (13, None),
            (0, Some(1)),
            (9, Some(3)),
            (3, Some(2)),
            (3, Some(2)),
        ] {
            assert_eq!(atom_row(&lines, &mut cursor, at), expected, "at={at}");
        }
        assert_eq!(atom_row(&[], &mut 0, 0), None);
    }
}
