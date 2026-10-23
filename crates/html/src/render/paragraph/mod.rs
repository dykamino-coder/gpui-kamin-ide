//! Абзац: строчные куски узла в элемент Paragraph, выравнивание атомов по строке.

use crate::layout::options::RenderOpts;
use crate::render::first_line_text;
use crate::render::paragraph::route::{NativeRequest, paragraph_pieces_routed};
use crate::text::shadow_text::normalize_for_shadow;
mod vertical;
pub(super) use vertical::vertical_paragraph;

mod alignment;

mod first_line_box;
mod metrics;

mod edges;
pub(super) use edges::own_size;

use crate::dom::Node;
use crate::hooks::IntoHooked;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

pub(crate) mod atom_piece;
pub(crate) mod route;

pub(crate) fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph_routed(nodes, inherited, opts, None)
}

fn paragraph_routed(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    native_request: Option<&NativeRequest<'_>>,
) -> AnyElement {
    // Vertical paragraphs choose the physical text route after inline collection.
    if inherited.wm().vertical() {
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
                Some(Len::Pct(k)) | Some(Len::Em(k)) => size * k,
                _ => crate::text::metrics::normal_px(
                    normal_fraction(&base, opts),
                    size,
                    opts.cx.vcx(),
                ),
            };
            let normal_font = crate::text::inline::strut_font(&base, &opts.text, opts.cx.vcx());
            let base_lh = base.line_height;
            let for_build = first.clone();
            // Абзац соберётся при раскладке, вне цикла `blocks_flow`: флаги
            // среза строк (`LayoutCx::trim_para`) едут с ним в `opts_owned`.
            let build: crate::layout::float::split_flow::Split =
                std::rc::Rc::new(move |at, width| {
                    let mut styled = base.clone();
                    styled.first_line = Some(for_build.clone());
                    let mut para =
                        paragraph_pieces(&nodes_owned, &styled, &opts_owned, at, &for_build);
                    para = div().w(width).child(para).into_any_element();
                    para
                });
            // Мерить надо ТЕМ начертанием, каким строка и будет набрана:
            // жирная первая строка занимает больше места, и разрез по
            // обычному шрифту не помещался бы в неё целиком.
            let font = crate::text::inline::strut_font(&first, &opts.text, opts.cx.vcx());
            let measure_size = match first.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * size,
                _ => size,
            };
            // Интерлиньяж первой строки: свой у `::first-line` или
            // унаследованный — числовой пересчитывается от её кегля.
            // The root inline inherits this strut, including a smaller height
            // (css-pseudo-4 §first-line-styling).
            let first_lh = match first.line_height.or(base_lh) {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * measure_size,
                _ => line * measure_size / size.max(1.0),
            };
            let mut full = String::new();
            first_line_text::lines(nodes, inherited.preserve_newlines == Some(true), &mut full);
            let following = full
                .split('\n')
                .skip(1)
                .map(|line| SharedString::from(normalize_for_shadow(line).trim().to_string()))
                .collect();
            return crate::layout::float::split_flow::FirstLine::new(
                build,
                SharedString::from(plain.trim().to_string()),
                font,
                measure_size,
                line,
                following,
                (normal_font, size),
            )
            .first_height(first_lh)
            .hooked();
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
        text_fit: c.text_fit.flatten(),
        hyphen_char: &c.hyphen_char,
    }
}
