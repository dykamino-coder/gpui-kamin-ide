//! The measure probe of a paragraph of a far block, kept across frames
//! (`layout::reuse::far`): once the block is known to be far, the whole probe
//! (`Paragraph`, ~1.5 KB of mostly empty feature fields and its preparation)
//! shrinks to what a new measure of a plain paragraph reads — its text and
//! runs, line and wrap settings, struts and its break opportunities — and a
//! fresh probe is made from that for each measure. Like Blink's
//! `NGInlineNodeData`, which keeps the items and shape results of the text
//! and not the layout objects of a pass.

mod settings;
mod stops;

use crate::text::paragraph::*;
pub(super) use settings::Settings;
use stops::PackedStops;

/// What the measure of a plain paragraph reads: its probe's settings, its
/// word and letter spacing spans and the break opportunities of its text.
pub(super) struct PlainProbe {
    settings: Settings,
    word_spans: Vec<(std::ops::Range<usize>, gpui::Pixels)>,
    letter_spans: Vec<(std::ops::Range<usize>, gpui::Pixels)>,
    stops: Option<PackedStops>,
}

impl PlainProbe {
    /// Probe `p` has a compact form (`of`): horizontal, with no per-span
    /// rules other than spacing, no atoms, ruby, clamp measure, fitting or
    /// fragmentation, so that `probe` makes `p` as its layout request built it.
    pub(super) fn plain(p: &Paragraph) -> bool {
        !p.vertical
            && p.vertical_inline.is_none()
            && p.clamp_rows.is_none()
            && p.ruby_base_sink.is_none()
            && p.ruby_layers.is_none()
            && p.spans.is_empty()
            && p.shift_spans.is_empty()
            && p.lh_spans.is_empty()
            && p.fit.is_none()
            && p.fragment.is_none()
            && p.atom_boxes.is_empty()
            && p.edge_spans.is_empty()
            && p.emph_spans.is_empty()
            && p.box_spans.is_empty()
            && p.spacers.is_empty()
            && p.spacer_edges.is_empty()
            && p.box_extents.is_empty()
            && p.atom_fit.borrow().is_empty()
    }

    /// The compact form of a plain probe `p` (`plain`).
    pub(super) fn of(mut p: Paragraph) -> Self {
        debug_assert!(Self::plain(&p));
        let stops = p.kept_stops().map(PackedStops::pack);
        let word_spans = std::mem::take(&mut p.word_spans);
        let letter_spans = std::mem::take(&mut p.letter_spans);
        Self {
            settings: Settings::from_probe(p),
            word_spans,
            letter_spans,
            stops,
        }
    }

    /// A fresh probe equal to the one the compact form was taken from, with
    /// the break opportunities its first measure seeds (`Paragraph::seed_stops`
    /// prepares the probe, which a measure does in any case).
    pub(super) fn probe(&self) -> (Paragraph, Option<Box<[u32]>>) {
        let mut p = self.settings.probe();
        p.word_spans = self.word_spans.clone();
        p.letter_spans = self.letter_spans.clone();
        (p, self.stops.as_ref().map(PackedStops::unpack))
    }
}
