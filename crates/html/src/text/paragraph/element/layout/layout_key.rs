//! Layout key of a paragraph's measured leaf (`gpui::Window::set_next_layout_key`).
//!
//! The window's frame tree keeps a measured leaf across frames when the new
//! request has the key of a previous one, and with the leaf its layout cache:
//! the line splits of an unchanged paragraph are not measured again on a
//! resize whose constraints reach it unchanged. The key therefore covers
//! every input of the measure probe (`request_layout_impl`). A paragraph whose
//! probe also reads state shared with other elements or bound later (atoms
//! without layout stamps, overlays, ruby, fragments, clamps, fitting,
//! vertical or rotated text) has no key and is measured in every frame.

use crate::text::paragraph::{AtomAlign, Paragraph};
use std::hash::{BuildHasher, Hash, Hasher};

/// The key's hasher: deterministic across frames and of hash-table quality,
/// several times cheaper per write than SipHash on the many small values of a
/// paragraph.
type KeyHasher = foldhash::quality::FoldHasher<'static>;

/// Formatter sink that feeds the hasher without building a string.
struct Sink<'a>(&'a mut KeyHasher);

impl std::fmt::Write for Sink<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}

impl Paragraph {
    /// Key of everything the measure probe reads, or `None` when the probe
    /// reads state a key cannot see. Atoms enter by the stamps of their
    /// layout roots: their sizes are in `atom_boxes`/`letter_spans`, and their
    /// shrink-to-fit widths come from memos of those unchanged subtrees.
    pub(super) fn layout_key(&self, window: &mut gpui::Window) -> Option<u64> {
        if !self.keyable() {
            return None;
        }
        let mut hasher = foldhash::quality::FixedState::default().build_hasher();
        self.text.as_ref().hash(&mut hasher);
        // Lists that grow with the atoms (one spacer span, one box and style
        // runs per atom) are hashed by their bits: formatting them made the
        // key cost microseconds per atom on pages with tens of thousands of
        // atoms. Float bits are at least as exact as round-trip `Debug`.
        for slot in &self.atoms {
            window.layout_stamp(slot.root.get()?)?.hash(&mut hasher);
        }
        for b in &self.atom_boxes {
            b.at.hash(&mut hasher);
            for v in [b.h, b.base, b.drop, b.over, b.under, b.shift] {
                v.to_bits().hash(&mut hasher);
            }
            match b.align {
                AtomAlign::Shift(v) => (0u8, v.to_bits()).hash(&mut hasher),
                AtomAlign::Middle => 1u8.hash(&mut hasher),
                AtomAlign::TextTop => 2u8.hash(&mut hasher),
                AtomAlign::TextBottom => 3u8.hash(&mut hasher),
                AtomAlign::Top => 4u8.hash(&mut hasher),
                AtomAlign::Bottom => 5u8.hash(&mut hasher),
            }
        }
        for spans in [
            &self.word_spans,
            &self.letter_spans,
            &self.shift_spans,
            &self.lh_spans,
        ] {
            spans.len().hash(&mut hasher);
            for (range, v) in spans {
                (range.start, range.end, f32::from(*v).to_bits()).hash(&mut hasher);
            }
        }
        // Style runs split at every atom spacer as well.
        self.runs.len().hash(&mut hasher);
        let px = |v: gpui::Pixels| f32::from(v).to_bits();
        let mut prev: Option<&gpui::TextRun> = None;
        for run in &self.runs {
            run.len.hash(&mut hasher);
            // Runs split at atoms repeat their style: an equal style enters as
            // one marker, not as its font again.
            let same = prev.is_some_and(|p| same_style(p, run));
            prev = Some(run);
            same.hash(&mut hasher);
            if same {
                continue;
            }
            // Exhaustive: a new run field must be added to the key.
            let gpui::TextRun {
                len: _,
                font,
                color,
                background_color,
                font_size,
                background_pad,
                background_radius,
                background_border,
                background_line_gap,
                underline,
                strikethrough,
            } = run;
            (font, color, background_color).hash(&mut hasher);
            (
                font_size.map(px),
                background_pad.map(px),
                px(*background_radius),
            )
                .hash(&mut hasher);
            background_border
                .map(|(color, widths)| (color, widths.map(px)))
                .hash(&mut hasher);
            (background_line_gap, underline, strikethrough).hash(&mut hasher);
        }
        self.run_metrics.len().hash(&mut hasher);
        for &(a, b) in &self.run_metrics {
            (a.to_bits(), b.to_bits()).hash(&mut hasher);
        }
        self.spans.hash(&mut hasher);
        // `Debug` of these values is exact (floats print round-trip).
        std::fmt::write(
            &mut Sink(&mut hasher),
            format_args!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                self.font_size,
                self.line_height,
                self.align,
                self.wrap,
                self.clamp_force,
                self.clamp_tag,
                self.tab_stop,
                self.indent,
                self.hanging,
                self.flow,
                self.first_strut,
                (self.strut, self.strut_box),
                (self.lines_reversed, self.ruby_trim),
            ),
        )
        .ok()?;
        Some(hasher.finish())
    }

    /// Whether the probe reads only the paragraph's own values and atoms.
    fn keyable(&self) -> bool {
        self.overlays.is_empty()
            && self.atom_boxes.len() == self.atoms.len()
            && self.ruby_base_sink.is_none()
            && self.ruby_layers.is_none()
            && !self.ruby_unit
            && !self.ruby_justify
            && self.fragment.is_none()
            && self.clamp_rows.is_none()
            && self.clamp.is_none()
            && self.fit.is_none()
            && self.inline_measure.is_none()
            && !self.vertical
            && self.vertical_inline.is_none()
            && self.ortho_limit.is_none()
            && !self.paint_context.is_rotated()
            && self.spacers.is_empty()
            && self.spacer_edges.is_empty()
            && self.box_extents.is_empty()
            && self.emph_spans.is_empty()
            && self.box_spans.is_empty()
            && self.edge_spans.is_empty()
    }
}

/// Whether two runs differ only in length.
fn same_style(a: &gpui::TextRun, b: &gpui::TextRun) -> bool {
    // Exhaustive: a new run field must be compared too.
    let gpui::TextRun {
        len: _,
        font,
        color,
        background_color,
        font_size,
        background_pad,
        background_radius,
        background_border,
        background_line_gap,
        underline,
        strikethrough,
    } = a;
    *font == b.font
        && *color == b.color
        && *background_color == b.background_color
        && *font_size == b.font_size
        && *background_pad == b.background_pad
        && *background_radius == b.background_radius
        && *background_border == b.background_border
        && *background_line_gap == b.background_line_gap
        && *underline == b.underline
        && *strikethrough == b.strikethrough
}

#[cfg(test)]
mod tests {
    use super::same_style;

    #[test]
    fn runs_that_differ_only_in_length_share_a_style() {
        let a = gpui::TextRun {
            len: 3,
            ..Default::default()
        };
        let b = gpui::TextRun {
            len: 5,
            ..a.clone()
        };
        assert!(same_style(&a, &b));
        let c = gpui::TextRun {
            color: gpui::hsla(0.5, 0.5, 0.5, 1.0),
            ..b.clone()
        };
        assert!(!same_style(&b, &c));
        let d = gpui::TextRun {
            font_size: Some(gpui::px(20.)),
            ..b.clone()
        };
        assert!(!same_style(&b, &d));
    }
}
