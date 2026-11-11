//! The settings every measure probe of a paragraph is made from.

use crate::text::paragraph::*;
use std::rc::Rc;

/// The fields of a layout probe that every probe takes from its paragraph
/// (`request_layout_impl`): text and runs, line settings, the first-line
/// indent, the `line-clamp` cut, struts and run metrics.
#[derive(Clone)]
pub(in crate::text::paragraph::element::layout) struct Settings {
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
    first_strut: Option<crate::text::paragraph::first_line::Strut>,
    run_metrics: Vec<(f32, f32)>,
    strut: (f32, f32, f32),
}

impl Settings {
    pub(in crate::text::paragraph::element::layout) fn of(p: &Paragraph) -> Self {
        Self {
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
        }
    }

    /// The fields of probe `p`, which its measures left as `probe` made them
    /// (a plain probe: no fitting or atoms rewrite its runs or struts).
    pub(super) fn from_probe(p: Paragraph) -> Self {
        Self {
            text_hash: p.text_hash(),
            text: p.text,
            runs: p.runs,
            font_size: p.font_size,
            line_height: p.line_height,
            align: p.align,
            wrap: p.wrap,
            ortho_limit: p.ortho_limit,
            clamp_tag: p.clamp_tag,
            clamp: p.clamp,
            clamp_force: p.clamp_force,
            measures: p.measures,
            paint_context: p.paint_context,
            inline_measure: p.inline_measure,
            lines_reversed: p.lines_reversed,
            vertical_central_baseline: p.vertical_central_baseline,
            vertical_ccw: p.vertical_ccw,
            tab_stop: p.tab_stop,
            indent: p.indent,
            hanging: p.hanging,
            flow: p.flow,
            ruby_trim: p.ruby_trim,
            ruby_unit: p.ruby_unit,
            ruby_justify: p.ruby_justify,
            strut_box: p.strut_box,
            first_strut: p.first_strut,
            run_metrics: p.run_metrics,
            strut: p.strut,
        }
    }

    /// A probe with these fields, every other one being `Paragraph::new`'s.
    pub(in crate::text::paragraph::element::layout) fn probe(&self) -> Paragraph {
        self.clone().into_probe()
    }

    /// The same probe, moving the fields instead of copying them.
    pub(in crate::text::paragraph::element::layout) fn into_probe(self) -> Paragraph {
        let mut p = Paragraph::new(
            self.text,
            self.runs,
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
        p.measures = self.measures;
        p.paint_context = self.paint_context;
        p.inline_measure = self.inline_measure;
        p.lines_reversed = self.lines_reversed;
        p.vertical_central_baseline = self.vertical_central_baseline;
        p.vertical_ccw = self.vertical_ccw;
        p.tab_stop = self.tab_stop;
        p.indent = self.indent;
        p.hanging = self.hanging;
        p.flow = self.flow;
        p.ruby_trim = self.ruby_trim;
        // Ruby's sub-line keeps trailing spaces and suppresses word
        // justification. Its intrinsic probe must use the same rules
        // as the paragraph that later places the base/annotation.
        p.ruby_unit = self.ruby_unit;
        p.ruby_justify = self.ruby_justify;
        p.strut_box = self.strut_box;
        p.first_strut = self.first_strut;
        p.run_metrics = self.run_metrics;
        p.strut = self.strut;
        p
    }
}
