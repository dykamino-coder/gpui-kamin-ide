//! Inline atoms round their absolute edges from the raw paragraph origin.

use super::*;

impl Paragraph {
    pub(super) fn place_atoms(&mut self, root: LayoutId, window: &mut Window, cx: &mut App) {
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
        // След мест атомов: ATOM_DBG=1.
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("ATOM_DBG").is_ok());
            *ON
        } {
            eprintln!(
                "ATOMS lh={lh} fs={:?} strut={:?} runs={:?} boxes={:?} pads={pads:?} bounds={bounds:?} fonts={:?} text={:?} lines={:?} emph={:?} trim={:?}",
                self.font_size,
                self.strut,
                self.run_metrics,
                self.atom_boxes,
                self.runs
                    .iter()
                    .map(|r| (r.font.family.clone(), r.font_size))
                    .collect::<Vec<_>>(),
                self.text,
                self.lines
                    .iter()
                    .map(|l| l.range.clone())
                    .collect::<Vec<_>>(),
                self.emph_spans,
                self.ruby_trim
            );
        }
        let mut tops = Vec::with_capacity(self.lines.len());
        let mut y = 0.0f32;
        for (i, _) in self.lines.iter().enumerate() {
            tops.push(y);
            let (p, q) = pads.get(i).copied().unwrap_or((0.0, 0.0));
            y += lh + p + q;
        }
        for k in 0..self.atoms.len() {
            let Some(b) = self.atom_boxes.get(k).copied() else {
                continue;
            };
            // Атом за концом последней строки остался в оборванном
            // `line-clamp` хвосте: прежде он вставал на последнюю видимую
            // строку поверх её текста (`line-clamp-auto-with-ruby-002`).
            let Some(row) = self.lines.iter().position(|l| b.at < l.range.end) else {
                self.atoms[k].hidden = true;
                continue;
            };
            self.atoms[k].hidden = false;
            let line = self.lines[row].clone();
            let (p, q) = pads.get(row).copied().unwrap_or((0.0, 0.0));
            let align = self.line_align(row, &line);
            let free_raw = bounds.size.width - line.width - line.indent - px(self.flow_cut(row).1);
            let hang = self.hang_first(line.range.start);
            let shift = self.span(&segs, line.range.start, line.range.start + hang);
            let lead = line.indent - shift;
            let dx = if align == Align::Justify {
                lead
            } else {
                line_offset(align, self.wrap.rtl, free_raw) + lead
            };
            let x = dx + self.x_at(&segs, b.at, Edge::Start)
                - self.x_at(&segs, line.range.start, Edge::Start);
            let top = match b.align {
                AtomAlign::Top => tops[row],
                AtomAlign::Bottom => tops[row] + lh + p + q - b.h,
                _ => tops[row] + p + self.line_base(&line.range) + self.atom_top(&b),
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
                        .prepaint_at(point(px(0.), px(0.)), window, cx);
                }
                None => {
                    self.atoms[k].el.prepaint_at(origin, window, cx);
                }
            }
        }
    }
}
