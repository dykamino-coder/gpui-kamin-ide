//! Типизированный ключ геометрии строк при обтекании: только данные,
//! которые меняют высоту строки и выбор свободного промежутка.

use crate::text::paragraph::*;
use std::hash::{Hash, Hasher};

impl Paragraph {
    pub(super) fn hash_line_geometry(&self, h: &mut impl Hasher) {
        f32::from(self.line_height).to_bits().hash(h);
        self.atom_boxes.len().hash(h);
        for b in &self.atom_boxes {
            b.at.hash(h);
            std::mem::discriminant(&b.align).hash(h);
            if let AtomAlign::Shift(v) = b.align {
                v.to_bits().hash(h);
            }
            for v in [b.h, b.base, b.over, b.under, b.shift] {
                v.to_bits().hash(h);
            }
        }
        for spans in [&self.shift_spans, &self.lh_spans] {
            spans.len().hash(h);
            for (range, value) in spans {
                range.hash(h);
                f32::from(*value).to_bits().hash(h);
            }
        }
        self.edge_spans.len().hash(h);
        for (range, top, height) in &self.edge_spans {
            range.hash(h);
            top.hash(h);
            height.to_bits().hash(h);
        }
        self.box_spans.len().hash(h);
        for (range, value) in &self.box_spans {
            range.hash(h);
            value.to_bits().hash(h);
        }
        self.emph_spans.len().hash(h);
        for span in &self.emph_spans {
            span.range.hash(h);
            span.ruby.is_some().hash(h);
            span.under.hash(h);
            span.size.to_bits().hash(h);
        }
        self.ruby_trim.start.hash(h);
        self.ruby_trim.end.hash(h);
        self.ruby_trim.over.to_bits().hash(h);
        (self.strut_box.0.to_bits(), self.strut_box.1.to_bits()).hash(h);
        (
            self.strut.0.to_bits(),
            self.strut.1.to_bits(),
            self.strut.2.to_bits(),
        )
            .hash(h);
        self.run_metrics.len().hash(h);
        for (a, b) in &self.run_metrics {
            a.to_bits().hash(h);
            b.to_bits().hash(h);
        }
    }
}
