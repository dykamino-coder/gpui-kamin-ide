//! `request_layout` абзаца: раскладка атомов и метрик прогонов ДО замера,
//! струт первой строки, и регистрация замеряемой раскладки у окна. Ширина
//! известна только раскладке, поэтому строки считаются в замере — в callback
//! уезжает копия абзаца со ВСЕМИ правилами кусков, обрывом `line-clamp`,
//! отступом и атомами (и общей памятью замера документа), иначе щуп отдаёт
//! другое число строк, чем отрисовка.

mod clamp;
mod layout_key;
mod measured_layout;
mod normal_static;
mod plain_probe;
mod probe_measure;
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
        self.invalidate_prepared();
        // Атомы раскладываются ДО замера: их ширина — продвижение распорки,
        // высота растит строку (см. `lay_atoms`).
        if !self.atoms.is_empty() {
            self.lay_atoms(window, cx);
        }
        // Рост строки под знак акцента меряется подъёмом и спуском его
        // прогона (`line_padding`); без замера при раскладке строка не росла
        // и рост доставался только сдвигу набора на отрисовке.
        let inline_anchor = self.overlays.iter().any(|(_, _, how)| {
            how.inline_box_info
                .as_ref()
                .is_some_and(|info| !info.names.is_empty())
        });
        if (!self.emph_spans.is_empty() || inline_anchor)
            && self.run_metrics.len() != self.runs.len()
        {
            self.refresh_run_metrics(window);
        }
        if !self.box_spans.is_empty() && self.run_metrics.len() != self.runs.len() {
            self.refresh_run_metrics(window);
        }
        // Root inline box metrics: boxed lines and the shared baseline of a
        // turned central line (`shared_base`) both start from them.
        if (!self.box_spans.is_empty() || self.rotated_central)
            && let Some((font, size)) = self.strut_run.clone()
        {
            let ts = window.text_system();
            let id = ts.resolve_font(&font);
            self.strut_box = crate::text::fonts::metric_overrides::extents(
                &font,
                f32::from(size),
                (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                ),
            );
        }
        self.measure_first_strut(window);
        self.invalidate_prepared();
        // Ширина известна только раскладке, поэтому строки считаются в замере:
        // сколько дали места — столько строк и получилось. Текст, прогоны,
        // строка, отступ первой строки, обрыв `line-clamp` и струты — общие
        // с ужатым щупом далёкого блока (`plain_probe::Settings`).
        let settings = plain_probe::Settings::of(self);
        let atom_boxes = self.atom_boxes.clone();
        let edge_spans = self.edge_spans.clone();
        let emph_spans = self.emph_spans.clone();
        let box_spans = self.box_spans.clone();
        let vertical = self.vertical;
        let vertical_inline = self.vertical_inline;
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
        let clamp_rows = self.clamp_rows.clone();
        let fit = self.fit;
        let spacers = self.spacers.clone();
        let spacer_edges = self.spacer_edges.clone();
        let box_extents = self.box_extents.clone();
        let fragment = self.fragment.clone();
        let atom_fit = self.atom_fit.clone();
        let ruby_base_sink = self.ruby_base_sink.clone();
        let ruby_layers = self.ruby_layers.clone();
        let normal_static = normal_static::NormalStatic::new(self, window, cx);
        let static_bind = normal_static.clone();
        let rebuild = fit.is_some() || !atom_fit.borrow().is_empty();
        // Completes a probe made from the settings (`plain_probe::Settings`).
        let make_probe = move |mut probe: Paragraph| {
            probe.clamp_rows = clamp_rows.clone();
            probe.vertical = vertical;
            probe.vertical_inline = vertical_inline;
            probe.ruby_base_sink = ruby_base_sink.clone();
            probe.spans = spans.clone();
            probe.word_spans = word_spans.clone();
            probe.letter_spans = letter_spans.clone();
            probe.shift_spans = shift_spans.clone();
            probe.lh_spans = lh_spans.clone();
            probe.fit = fit;
            probe.fragment = fragment.clone();
            probe.atom_boxes = atom_boxes.clone();
            probe.edge_spans = edge_spans.clone();
            probe.ruby_layers = ruby_layers.clone();
            probe.emph_spans = emph_spans.clone();
            probe.box_spans = box_spans.clone();
            probe.spacers = spacers.clone();
            probe.spacer_edges = spacer_edges.clone();
            probe.box_extents = box_extents.clone();
            probe
        };
        // Узел раскладки держит замер до конца кадра: готовый щуп и поля для
        // его пересборки не хранятся вместе, место берёт только один из них.
        if let Some(key) = self.layout_key(window) {
            window.set_next_layout_key(key);
        }
        let source = if rebuild {
            probe_measure::Probe::Rebuild(settings, make_probe)
        } else {
            probe_measure::Probe::Built(make_probe(settings.into_probe()))
        };
        // A far block's kept measure shrinks to its plain form
        // (`probe_measure::ProbeMeasure::compact`).
        let id = window.request_measured_layout_with_measure(
            gpui::Style::default(),
            probe_measure::ProbeMeasure::new(source, atom_fit, normal_static),
        );
        static_bind.bind(crate::layout::fragment::geometry::Node::capture(id, window));
        if let Some(f) = &self.fragment {
            f.node.record(id, window);
        }
        (id, id)
    }
}
