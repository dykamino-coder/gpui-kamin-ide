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
use std::cell::Cell;
use std::rc::Rc;
use stops::PackedStops;

/// A measure probe that a far block may compact (`Compact`).
pub(super) struct Shared(Cell<Option<State>>);

enum State {
    /// The built probe: compaction moves its settings out of it, so a probe
    /// that is never compacted (a block near the screen) holds one copy.
    Built(Box<Paragraph>),
    Plain(PlainProbe),
}

impl Shared {
    pub(super) fn new(probe: Paragraph) -> Rc<Self> {
        Rc::new(Self(Cell::new(Some(State::Built(Box::new(probe))))))
    }

    /// `f` with the probe: the built one, or one made from the compact form.
    pub(super) fn measure<R>(&self, f: impl FnOnce(&mut Paragraph) -> R) -> R {
        let mut state = self.0.take().expect("a measure probe is not re-entered");
        let out = match &mut state {
            State::Built(probe) => f(probe),
            State::Plain(plain) => f(&mut plain.probe()),
        };
        self.0.set(Some(state));
        out
    }
}

impl crate::layout::reuse::compact::Compact for Shared {
    fn compact(&self) {
        let state = match self.0.take() {
            Some(State::Built(probe)) => match PlainProbe::of(probe) {
                Ok(plain) => State::Plain(plain),
                Err(probe) => State::Built(probe),
            },
            other => other.expect("a measure probe is not re-entered"),
        };
        self.0.set(Some(state));
    }
}
/// What the measure of a plain paragraph reads: its probe's settings, its
/// word and letter spacing spans and the break opportunities of its text.
struct PlainProbe {
    settings: Box<Settings>,
    word_spans: Vec<(std::ops::Range<usize>, gpui::Pixels)>,
    letter_spans: Vec<(std::ops::Range<usize>, gpui::Pixels)>,
    stops: Option<PackedStops>,
}

impl PlainProbe {
    /// The compact form of probe `p`, if it is plain: horizontal, with no
    /// per-span rules other than spacing, no atoms, ruby, clamp measure,
    /// fitting or fragmentation, so that `probe` makes `p` as its layout
    /// request built it. Otherwise `p` is given back.
    fn of(mut p: Box<Paragraph>) -> Result<Self, Box<Paragraph>> {
        let plain = !p.vertical
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
            && p.atom_fit.borrow().is_empty();
        if !plain {
            return Err(p);
        }
        let stops = p.kept_stops().map(PackedStops::pack);
        let word_spans = std::mem::take(&mut p.word_spans);
        let letter_spans = std::mem::take(&mut p.letter_spans);
        Ok(Self {
            settings: Box::new(Settings::from_probe(*p)),
            word_spans,
            letter_spans,
            stops,
        })
    }

    /// A fresh probe equal to the one the compact form was taken from.
    fn probe(&self) -> Paragraph {
        let mut p = self.settings.probe();
        p.word_spans = self.word_spans.clone();
        p.letter_spans = self.letter_spans.clone();
        if let Some(stops) = &self.stops {
            p.seed_stops(stops.unpack());
        }
        p
    }
}
