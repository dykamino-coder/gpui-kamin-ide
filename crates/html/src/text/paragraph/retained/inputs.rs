//! Полные входы набора: сравниваем поток полей, а не только его хеш.

use super::super::Paragraph;
use gpui::{SharedString, TextRun};
use std::hash::{Hash, Hasher};

pub(super) struct Inputs {
    text: SharedString,
    fields: Vec<u8>,
}

impl Inputs {
    pub(super) fn shape(p: &Paragraph) -> Self {
        Self::capture(p, |h| shape(p, h))
    }

    pub(super) fn lines(p: &Paragraph) -> Self {
        Self::capture(p, |h| lines(p, h))
    }

    fn capture(p: &Paragraph, emit: impl FnOnce(&mut Stream<'_>)) -> Self {
        let mut fields = Vec::new();
        emit(&mut Stream::Store(&mut fields));
        Self {
            text: p.text.clone(),
            fields,
        }
    }

    pub(super) fn matches_shape(&self, p: &Paragraph) -> bool {
        self.matches(p, |h| shape(p, h))
    }

    pub(super) fn matches_lines(&self, p: &Paragraph) -> bool {
        self.matches(p, |h| lines(p, h))
    }

    fn matches(&self, p: &Paragraph, emit: impl FnOnce(&mut Stream<'_>)) -> bool {
        if self.text != p.text {
            return false;
        }
        let mut check = Stream::Check {
            remaining: &self.fields,
            equal: true,
        };
        emit(&mut check);
        matches!(
            check,
            Stream::Check {
                remaining: [],
                equal: true
            }
        )
    }

    #[cfg(feature = "heap-report")]
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        use kamin_heap::HeapSize;
        self.text.heap_size(ops) + ops.buffer(self.fields.as_ptr(), self.fields.capacity())
    }
}

/// Границы write сохраняются: разные последовательности полей не склеиваются.
enum Stream<'a> {
    Store(&'a mut Vec<u8>),
    Check { remaining: &'a [u8], equal: bool },
}

impl Hasher for Stream<'_> {
    fn finish(&self) -> u64 {
        unreachable!("input stream is never reduced to a hash")
    }
    fn write(&mut self, bytes: &[u8]) {
        let len = bytes.len().to_ne_bytes();
        for part in [len.as_slice(), bytes] {
            match self {
                Self::Store(out) => out.extend_from_slice(part),
                Self::Check { remaining, equal } => {
                    if remaining.starts_with(part) {
                        *remaining = &remaining[part.len()..];
                    } else {
                        *equal = false;
                    }
                }
            }
        }
    }
}

fn shape(p: &Paragraph, h: &mut impl Hasher) {
    f32::from(p.font_size).to_bits().hash(h);
    f32::from(p.letter_spacing).to_bits().hash(h);
    f32::from(p.word_spacing).to_bits().hash(h);
    p.joined_spacing().hash(h);
    [p.wrap.rtl, p.vertical].hash(h);
    p.tab_stop.spans.len().hash(h);
    p.tab_stop.hash_into(h);
    fonts(&p.runs, h);
    for spans in [&p.word_spans, &p.letter_spans] {
        spans.len().hash(h);
        for (range, value) in spans {
            range.hash(h);
            f32::from(*value).to_bits().hash(h);
        }
    }
}

fn lines(p: &Paragraph, h: &mut impl Hasher) {
    p.wrap.hash(h);
    [p.ruby_unit, p.ruby_justify].hash(h);
    p.spans.hash(h);
    p.indent.px.to_bits().hash(h);
    p.indent.pct.to_bits().hash(h);
    p.indent.each_line.hash(h);
    p.indent.hanging.hash(h);
    let hg = &p.hanging;
    [hg.first, hg.last, hg.force_end, hg.allow_end].hash(h);
    p.clamp.hash(h);
    p.clamp_force.hash(h);
    p.text_overflow.hash(h);
    p.overflow_marker.hash(h);
    p.clamp_marker.hash(h);
    p.marker_size.map(|s| f32::from(s).to_bits()).hash(h);
    p.marker_font.hash(h);
    p.hyphen.hash(h);
    p.hyphen_limit_lines.hash(h);
    p.spacers.hash(h);
}

fn fonts(runs: &[TextRun], h: &mut impl Hasher) {
    runs.len().hash(h);
    for run in runs {
        run.len.hash(h);
        run.font_size.map(|s| f32::from(s).to_bits()).hash(h);
        // Includes fallback order and stretch, not paint-only decorations.
        run.font.hash(h);
    }
}

pub(in crate::text::paragraph) fn font_fields(runs: &[TextRun]) -> Vec<u8> {
    let mut out = Vec::new();
    fonts(runs, &mut Stream::Store(&mut out));
    out
}

pub(super) fn matches_fonts(fields: &[u8], runs: &[TextRun]) -> bool {
    let mut stream = Stream::Check {
        remaining: fields,
        equal: true,
    };
    fonts(runs, &mut stream);
    matches!(
        stream,
        Stream::Check {
            remaining: [],
            equal: true
        }
    )
}
