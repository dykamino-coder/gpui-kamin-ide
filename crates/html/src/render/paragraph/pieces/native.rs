//! Построение абзаца с собственной раскладкой строк.

use super::paragraph_spans;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, SharedString};

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_paragraph(
    inherited: &Computed,
    opts: &RenderOpts,
    native_request: Option<&native_paragraph_route::Request<'_>>,
    native: bool,
    selectable: bool,
    text: String,
    runs: Vec<gpui::TextRun>,
    pieces: Vec<inline::Piece>,
    edges: Vec<(std::ops::Range<usize>, bool, f32)>,
    boxes: Option<(Vec<(std::ops::Range<usize>, f32)>, f32)>,
    biggest: f32,
    wrap: crate::text::paragraph::Wrap,
    indent: crate::text::paragraph::Indent,
    clamp_budget: Option<usize>,
    clamp_tag: Option<(u64, u32)>,
    line_atoms: Vec<(
        usize,
        AnyElement,
        crate::text::paragraph::AtomAlign,
        crate::text::paragraph::RubyExtents,
    )>,
) -> AnyElement {
    let inherited = if native {
        native_request.unwrap().style
    } else {
        inherited
    };
    // Кегль абзаца — самый крупный кусок в нём: строка растёт под него,
    // и от него же считается высота строки в долях.
    //
    // ПРОБОВАЛИ И ОТКАТИЛИ: считать долю от кегля САМОГО блока
    // (струт §10.8), раз крупный кусок теперь растит строку каналом
    // `lh_spans`. Замерено: приобретено 4, потеряно 4 — три пары
    // `*-applies-to-008` уходят с 0.02 на 0.67. Возвращать вместе с
    // разбором `vertical-align: top/bottom` на тексте.
    // Куски у края строки в её струт не входят (§10.8.1): у
    // `vertical-align-121` строка из 30px текста и прижатого вверх
    // 60px куска — это 30px струта плюс вылет куска вниз, а не 60px
    // с текстом посередине.
    let (flow_biggest, flow_lh) = if edges.is_empty() {
        (biggest, 0.0)
    } else {
        flow_metrics(&pieces, &edges, inherited, opts)
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => gpui::px(v),
        Some(Len::Pct(k)) => gpui::px(k * flow_biggest),
        Some(Len::Em(k)) => gpui::px(k * flow_biggest),
        _ if !edges.is_empty() => gpui::px(flow_lh),
        // Своей `line-height` у блока нет — её задают КУСКИ: у куска
        // со своей высотой строки она и берётся, у остальных доля от
        // кегля (§10.8.1). Канал `lh_spans` умеет строку только
        // растить, и объявленная `font: 100px/1` терялась.
        _ => gpui::px(inline::max_line_height(
            &pieces,
            own_size(inherited, opts),
            opts.base_size(),
            normal_fraction(inherited, opts),
        )),
    };
    // Построчные коробки (`line_box_spans`): высота строки абзаца — СТРУТ
    // блока, крупные куски растят только свои строки.
    let line = match &boxes {
        Some((_, strut)) => gpui::px(*strut),
        None => line,
    };
    let id = gpui::ElementId::Integer(text_id(&text));
    let family = inherited.font_family.clone().unwrap_or_default();
    let para = crate::text::paragraph::Paragraph::new(
        SharedString::from(text),
        runs,
        gpui::px(biggest),
        line,
        crate::text::paragraph::align_for(inherited),
        wrap,
    );
    let para = paragraph_spans(para, inherited, opts, &pieces, &edges, biggest, line)
        .edge_spans(edges)
        .line_boxes(
            boxes.as_ref().map(|b| b.0.clone()).unwrap_or_default(),
            boxes.as_ref().map(|_| {
                (
                    inline::strut_font(inherited, &opts.text),
                    gpui::px(own_size(inherited, opts)),
                )
            }),
        )
        .rel_spans(inline::rel_spans(&pieces))
        .ruby_justify(inherited.ruby_justify == Some(true), inherited.ruby_unit)
        .justify_chars(inherited.justify_chars.unwrap_or(1))
        .align_last(
            inherited
                .text_align_last
                .map(|a| a.physical(inherited.rtl == Some(true)))
                .map(crate::text::paragraph::align_of_value)
                // `text-justify: none` forbids justification of the last
                // line too: it aligns as `start` (css-text-3 §7.3,
                // `text-justify-none-001` with `text-align-last: justify`).
                .map(|a| match a {
                    crate::text::paragraph::Align::Justify
                        if inherited.no_justify == Some(true) =>
                    {
                        if inherited.rtl == Some(true) {
                            crate::text::paragraph::Align::Right
                        } else {
                            crate::text::paragraph::Align::Left
                        }
                    }
                    other => other,
                }),
        )
        .letter_spacing(gpui::px(crate::text::metrics::spacing_px(
            inherited.letter_spacing,
            &family,
            biggest,
        )))
        .word_spacing(gpui::px(crate::text::metrics::spacing_px(
            inherited.word_spacing,
            &family,
            biggest,
        )))
        .hanging(inherited.hanging)
        .indent(indent)
        .spacers(inline::spacers(&pieces))
        .spacer_edges(inline::spacer_edges(&pieces))
        .box_extents(inline::box_extents(&pieces))
        .flow_shapes(
            inherited
                .flow_shapes
                .clone()
                .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new()))),
        )
        // Счётный режим (`line-clamp: <N>`) берёт предел из стиля;
        // авто-режим — из бюджета, посчитанного по точке среза.
        .line_clamp(clamp_budget.or_else(|| inherited.clamp_lines().map(|n| n as usize)))
        .clamp_marked(clamp_budget.is_some())
        .text_ellipsis(
            inherited.ellipsis == Some(true)
                && inherited
                    .overflow_x
                    .is_some_and(|o| o != crate::style::computed::Overflow::Visible),
        )
        .overflow_marker(
            inherited.overflow_marker.clone(),
            Some(measure_font(inherited, opts)),
            Some(gpui::px(own_size(inherited, opts))),
        )
        .clamp_mark(inherited.clamp_mark.clone())
        .clamp_tag(clamp_tag)
        // Знак обрыва — анонимный строчный ребёнок блока: и
        // `visibility` у него блочная (css-overflow-3 §text-overflow,
        // css-overflow-4 §block-ellipsis): у скрытого блока знака не
        // видно, даже если кусок у среза `visible`
        // (`text-overflow-ellipsis-002`, `webkit-line-clamp-035`).
        .marker_color(Some(if inherited.hidden == Some(true) {
            gpui::transparent_black()
        } else {
            inherited
                .color
                .map(crate::style::values::value::Color::to_hsla)
                .unwrap_or_else(gpui::black)
        }))
        .text_fit(crate::render::paragraph::paragraph_style(inherited).text_fit)
        .fit_parts(
            // Масштабируемы только интервалы в ДОЛЯХ кегля; `px` и `em`
            // (от вычисленного кегля) подбор не трогает.
            [inherited.letter_spacing, inherited.word_spacing]
                .iter()
                .all(|l| matches!(l, None | Some(Len::Pct(_)))),
            matches!(
                inherited.line_height,
                Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Ex(_)) | Some(Len::Ch(_))
            ),
        )
        .hyphen_char(
            crate::render::paragraph::paragraph_style(inherited)
                .hyphen_char
                .clone(),
        )
        .tab_stops(inline::tab_stops(&pieces, inherited, &opts.text))
        .overlays(inline::overlays(pieces))
        .atoms(line_atoms)
        .ruby_trim(inherited.text_box_trim_start, inherited.text_box_trim_end);
    let para = if selectable {
        para.selectable(id, opts.selection_color())
    } else {
        para
    };
    if native {
        native_request.unwrap().built.set(true);
    }
    para.into_any_element()
}
