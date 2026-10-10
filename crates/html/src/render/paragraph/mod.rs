//! Абзац: строчные куски узла в элемент Paragraph, выравнивание атомов по строке.

mod vertical;
pub(super) use vertical::vertical_paragraph;

mod alignment;
pub(super) use alignment::atom_line_align;

mod metrics;
pub(super) use metrics::atoms_fit_line;
pub(super) use metrics::flow_metrics;
pub(super) use metrics::line_box_spans;

mod edges;
pub(super) use edges::edge_pieces;
pub(super) use edges::has_flow_text;
pub(super) use edges::in_edge;
pub(super) use edges::own_size;

use crate::dom::Node;
use crate::paint::effects::paint_scope::DepthScope;
use crate::paint::effects::paint_scope::snapshot as defer_depth;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

pub(crate) mod atom_piece;
pub(super) mod pieces;

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
/// Абзац для тех, кто собирает текст сам — содержимое поля ввода.
pub fn paragraph_public(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph(nodes, inherited, opts)
}

pub(crate) fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph_routed(nodes, inherited, opts, None)
}

fn paragraph_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    native_request: Option<&native_paragraph_route::Request<'_>>,
) -> AnyElement {
    // Vertical paragraphs choose the physical text route after inline collection.
    if inherited.vertical == Some(true) {
        return vertical_paragraph(nodes, inherited, opts, native_request);
    }
    // Первая строка со своим стилем: где она кончается, известно только после
    // переноса, поэтому абзац собирается замером (см. `float::FirstLine`).
    if let Some(first) = inherited.first_line.clone() {
        let mut base = inherited.clone();
        base.first_line = None;
        let nodes_owned = nodes.to_vec();
        let opts_owned = opts.clone();
        let mut plain = String::new();
        first_line_text::gather(nodes, inherited.preserve_newlines == Some(true), &mut plain);
        let plain = crate::text::inline::transform_case(&normalize_for_shadow(&plain), inherited);
        if !plain.trim().is_empty() {
            let size = match base.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => opts.base_size(),
            };
            let line = match base.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => size * k,
                _ => size * normal_fraction(&base, opts),
            };
            let for_build = first.clone();
            let depth = defer_depth();
            let build: crate::layout::float::split_flow::Split =
                std::rc::Rc::new(move |at, width| {
                    let _depth = DepthScope::enter(depth);
                    let mut styled = base.clone();
                    styled.first_line = None;
                    let mut para =
                        paragraph_pieces(&nodes_owned, &styled, &opts_owned, at, &for_build);
                    para = div().w(width).child(para).into_any_element();
                    para
                });
            // Мерить надо ТЕМ начертанием, каким строка и будет набрана:
            // жирная первая строка занимает больше места, и разрез по
            // обычному шрифту не помещался бы в неё целиком.
            let mut font = opts.text.font();
            font.fallbacks = crate::style::computed::font_family::fallbacks(&first, font.fallbacks);
            if let Some(w) = first.font_weight {
                font.weight = gpui::FontWeight(w as f32);
            }
            if first.italic == Some(true) {
                font.style = gpui::FontStyle::Italic;
            }
            if let Some(family) = first.font_family.as_ref().filter(|f| !f.is_empty()) {
                font.family = family.clone().into();
            }
            let measure_size = match first.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * size,
                _ => size,
            };
            return crate::layout::float::split_flow::FirstLine::new(
                build,
                SharedString::from(plain.trim().to_string()),
                font,
                measure_size,
                line,
            )
            .into_any_element();
        }
    }
    paragraph_pieces_routed(
        nodes,
        inherited,
        opts,
        0,
        &Computed::default(),
        native_request,
    )
}

/// Абзац с готовым разрезом первой строки: `at` — сколько байт в неё вошло.
fn paragraph_pieces(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
) -> AnyElement {
    paragraph_pieces_routed(nodes, inherited, opts, first_line_at, first_line, None)
}

/// Параметры маршрута и набора абзаца; строка слогораздела остаётся за стилем.
pub(super) struct ParagraphStyle<'a> {
    hug_claim: bool,
    lines_reversed: Option<bool>,
    text_fit: Option<crate::style::computed::TextFit>,
    hyphen_char: &'a Option<String>,
}

pub(super) fn paragraph_style(c: &Computed) -> ParagraphStyle<'_> {
    ParagraphStyle {
        hug_claim: c.hug_claim,
        lines_reversed: c.lines_reversed,
        text_fit: c.text_fit,
        hyphen_char: &c.hyphen_char,
    }
}
