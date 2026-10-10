//! Layout for element; split out to keep the owning module within 250 lines.

mod measured_layout;
pub(crate) use crate::text::paragraph::element::layout::measured_layout::measured_layout;

use crate::text::paragraph::*;
use gpui::{App, GlobalElementId, InspectorElementId, LayoutId, Window};

impl Paragraph {
    pub(crate) fn request_layout_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        // Атомы раскладываются ДО замера: их ширина — продвижение распорки,
        // высота растит строку (см. `lay_atoms`).
        if !self.atoms.is_empty() {
            self.lay_atoms(window, cx);
        }
        // Рост строки под знак акцента меряется подъёмом и спуском его
        // прогона (`line_padding`); без замера при раскладке строка не росла
        // и рост доставался только сдвигу набора на отрисовке.
        if !self.emph_spans.is_empty() && self.run_metrics.len() != self.runs.len() {
            self.run_metrics = self.measure_runs(window);
        }
        if !self.box_spans.is_empty() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            if let Some((font, size)) = self.strut_run.clone() {
                let ts = window.text_system();
                let id = ts.resolve_font(&font);
                self.strut_box = (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                );
            }
        }
        let atom_boxes = self.atom_boxes.clone();
        let edge_spans = self.edge_spans.clone();
        let ruby_trim = self.ruby_trim;
        let emph_spans = self.emph_spans.clone();
        let box_spans = self.box_spans.clone();
        let strut_box = self.strut_box;
        let run_metrics = self.run_metrics.clone();
        let strut = self.strut;
        // Ширина известна только раскладке, поэтому строки считаются в замере:
        // сколько дали места — столько строк и получилось.
        let text = self.text.clone();
        let runs = self.runs.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let wrap = self.wrap;
        let align = self.align;
        let vertical = self.vertical;
        let lines_reversed = self.lines_reversed;
        let vertical_central_baseline = self.vertical_central_baseline;
        let vertical_ccw = self.vertical_ccw;
        let vertical_inline = self.vertical_inline;
        let ortho_limit = self.ortho_limit;
        // Правила КУСКОВ обязаны доехать и до замера: без них щуп считает
        // абзац по общим правилам и отдаёт другое число строк, чем потом
        // рисуется. Коробка тогда выходит по замеру, а текст по отрисовке —
        // и лишние строки вылезают за рамку (`white-space-pre-031`).
        let spans = self.spans.clone();
        let word_spans = self.word_spans.clone();
        let letter_spans = self.letter_spans.clone();
        let shift_spans = self.shift_spans.clone();
        let lh_spans = self.lh_spans.clone();
        // Обрыв по `line-clamp` обязан доехать и до замера: иначе коробка
        // считается по ПОЛНОМУ числу строк, а рисуются обрезанные, и рамка
        // выходит выше текста (`text-wrap-balance-line-clamp-004`).
        let clamp = self.clamp;
        let clamp_force = self.clamp_force;
        let fit = self.fit;
        let tab_stop = self.tab_stop.clone();
        // Отступ первой строки решает и число строк, и ширину коробки —
        // без него щуп мерил абзац по чужой раскладке.
        let indent = self.indent;
        let hanging = self.hanging;
        let spacers = self.spacers.clone();
        let spacer_edges = self.spacer_edges.clone();
        let box_extents = self.box_extents.clone();
        let flow = self.flow.clone();
        let atom_fit = self.atom_fit.clone();
        let ruby_base_sink = self.ruby_base_sink.clone();
        let id = window.request_measured_layout_with_physical_baselines(
            gpui::Style::default(),
            move |known, available, window, _cx| {
                // Заданная ширина сильнее доступной: раскладка уже решила, в
                // какую коробку абзац ставится, и переносы считаются по ней.
                let mut probe = Paragraph::new(
                    text.clone(),
                    runs.clone(),
                    font_size,
                    line_height,
                    align,
                    wrap,
                );
                probe.vertical = vertical;
                probe.lines_reversed = lines_reversed;
                probe.vertical_central_baseline = vertical_central_baseline;
                probe.vertical_ccw = vertical_ccw;
                probe.vertical_inline = vertical_inline;
                probe.spans = spans.clone();
                probe.word_spans = word_spans.clone();
                probe.letter_spans = letter_spans.clone();
                probe.shift_spans = shift_spans.clone();
                probe.lh_spans = lh_spans.clone();
                probe.clamp = clamp;
                probe.clamp_force = clamp_force;
                probe.fit = fit;
                probe.tab_stop = tab_stop.clone();
                probe.indent = indent;
                probe.hanging = hanging;
                probe.flow = flow.clone();
                probe.atom_boxes = atom_boxes.clone();
                probe.edge_spans = edge_spans.clone();
                probe.ruby_trim = ruby_trim;
                probe.emph_spans = emph_spans.clone();
                probe.box_spans = box_spans.clone();
                probe.strut_box = strut_box;
                probe.run_metrics = run_metrics.clone();
                probe.strut = strut;
                probe.spacers = spacers.clone();
                probe.spacer_edges = spacer_edges.clone();
                probe.box_extents = box_extents.clone();
                measured_layout(
                    probe,
                    atom_fit.clone(),
                    known,
                    available,
                    vertical,
                    vertical_inline.is_some(),
                    ortho_limit,
                    line_height,
                    &ruby_base_sink,
                    window,
                    _cx,
                )
            },
        );
        (id, id)
    }
}
