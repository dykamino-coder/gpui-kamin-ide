//! The measure probe of a paragraph of a far block, kept across frames
//! (`layout::reuse::far`): once the block is known to be far, the whole probe
//! (`Paragraph`, ~1.5 KB of mostly empty feature fields and its preparation)
//! shrinks to what a new measure of a plain paragraph reads — its text and
//! runs, line and wrap settings, struts and its break opportunities — and a
//! fresh probe is made from that for each measure. Like Blink's
//! `NGInlineNodeData`, which keeps the items and shape results of the text
//! and not the layout objects of a pass.

use crate::text::paragraph::*;
use std::cell::RefCell;
use std::rc::Rc;

/// A measure probe that a far block may compact (`Compact`).
pub(super) struct Shared(RefCell<State>);

enum State {
    Built(Box<Paragraph>),
    Plain(Box<PlainProbe>),
}

impl Shared {
    pub(super) fn new(probe: Paragraph) -> Rc<Self> {
        Rc::new(Self(RefCell::new(State::Built(Box::new(probe)))))
    }

    /// `f` with the probe: the built one, or one made from the compact form.
    pub(super) fn measure<R>(&self, f: impl FnOnce(&mut Paragraph) -> R) -> R {
        let mut state = self.0.borrow_mut();
        match &mut *state {
            State::Built(probe) => f(probe),
            State::Plain(plain) => f(&mut plain.probe()),
        }
    }
}

impl crate::layout::reuse::compact::Compact for Shared {
    fn compact(&self) {
        let mut state = self.0.borrow_mut();
        if let State::Built(probe) = &*state
            && let Some(plain) = PlainProbe::of(probe)
        {
            *state = State::Plain(Box::new(plain));
        }
    }
}

/// What the measure of a plain paragraph reads: the fields its layout probe
/// gets (`request_layout_impl`), every other one being `Paragraph::new`'s.
struct PlainProbe {
    text: gpui::SharedString,
    text_hash: u64,
    runs: Vec<gpui::TextRun>,
    font_size: gpui::Pixels,
    line_height: gpui::Pixels,
    align: Align,
    wrap: Wrap,
    ortho_limit: Option<gpui::Pixels>,
    clamp_tag: Option<(u64, u32)>,
    clamp: Option<usize>,
    clamp_force: bool,
    measures: Rc<MeasureCache>,
    paint_context: Rc<crate::text::vertical::VerticalPaint>,
    inline_measure: Option<crate::text::vertical::InlineMeasure>,
    lines_reversed: bool,
    vertical_central_baseline: bool,
    vertical_ccw: bool,
    tab_stop: crate::text::paragraph::tabs::TabStops,
    indent: Indent,
    hanging: crate::style::computed::Hanging,
    flow: std::sync::Arc<(
        Vec<crate::layout::float::shapes::FloatShape>,
        Vec<crate::layout::float::shapes::FloatShape>,
    )>,
    ruby_trim: crate::text::line_trim::Trim,
    ruby_unit: bool,
    ruby_justify: bool,
    strut_box: (f32, f32),
    first_strut: Option<super::super::super::first_line::Strut>,
    run_metrics: Vec<(f32, f32)>,
    strut: (f32, f32, f32),
    /// Break opportunities of the text (`Prepared::stops`), if found.
    stops: Option<Box<[u32]>>,
}

impl PlainProbe {
    /// The compact form of probe `p`, if it is plain: horizontal, with no
    /// per-span rules, atoms, ruby, clamp measure, fitting or fragmentation,
    /// so that a probe made from it is `p` as its layout request built it.
    fn of(p: &Paragraph) -> Option<Self> {
        let plain = !p.vertical
            && p.vertical_inline.is_none()
            && p.clamp_rows.is_none()
            && p.ruby_base_sink.is_none()
            && p.ruby_layers.is_none()
            && p.spans.is_empty()
            && p.word_spans.is_empty()
            && p.letter_spans.is_empty()
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
        plain.then(|| Self {
            text: p.text.clone(),
            text_hash: p.text_hash(),
            runs: p.runs.clone(),
            font_size: p.font_size,
            line_height: p.line_height,
            align: p.align,
            wrap: p.wrap,
            ortho_limit: p.ortho_limit,
            clamp_tag: p.clamp_tag,
            clamp: p.clamp,
            clamp_force: p.clamp_force,
            measures: p.measures.clone(),
            paint_context: p.paint_context.clone(),
            inline_measure: p.inline_measure.clone(),
            lines_reversed: p.lines_reversed,
            vertical_central_baseline: p.vertical_central_baseline,
            vertical_ccw: p.vertical_ccw,
            tab_stop: p.tab_stop.clone(),
            indent: p.indent,
            hanging: p.hanging,
            flow: p.flow.clone(),
            ruby_trim: p.ruby_trim,
            ruby_unit: p.ruby_unit,
            ruby_justify: p.ruby_justify,
            strut_box: p.strut_box,
            first_strut: p.first_strut.clone(),
            run_metrics: p.run_metrics.clone(),
            strut: p.strut,
            stops: p.kept_stops().map(Box::from),
        })
    }

    /// A fresh probe equal to the one the compact form was taken from.
    fn probe(&self) -> Paragraph {
        let mut p = Paragraph::new(
            self.text.clone(),
            self.runs.clone(),
            self.font_size,
            self.line_height,
            self.align,
            self.wrap,
        )
        .ortho_limit(self.ortho_limit);
        p.prepared.seed_text_hash(self.text_hash);
        p.clamp_tag = self.clamp_tag;
        p.clamp = self.clamp;
        p.clamp_force = self.clamp_force;
        p.measures = self.measures.clone();
        p.paint_context = self.paint_context.clone();
        p.inline_measure = self.inline_measure.clone();
        p.lines_reversed = self.lines_reversed;
        p.vertical_central_baseline = self.vertical_central_baseline;
        p.vertical_ccw = self.vertical_ccw;
        p.tab_stop = self.tab_stop.clone();
        p.indent = self.indent;
        p.hanging = self.hanging;
        p.flow = self.flow.clone();
        p.ruby_trim = self.ruby_trim;
        p.ruby_unit = self.ruby_unit;
        p.ruby_justify = self.ruby_justify;
        p.strut_box = self.strut_box;
        p.first_strut = self.first_strut.clone();
        p.run_metrics = self.run_metrics.clone();
        p.strut = self.strut;
        if let Some(stops) = &self.stops {
            p.seed_stops(stops.clone());
        }
        p
    }
}
