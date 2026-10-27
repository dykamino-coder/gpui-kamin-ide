//! Построение абзаца с собственной раскладкой строк.

use super::spans::{paragraph_spans, rotate_rel_spans, rotated_central};
use super::{LineAtoms, NativeRequest};
use crate::layout::options::{RenderOpts, own_size};
use crate::render::paragraph::metrics::flow_metrics;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use crate::text::{
    fonts::style_font::measure_font, inline, metrics, paragraph::text_id::text_id,
    text_box::normal_fraction,
};
use gpui::{AnyElement, IntoElement, SharedString};
#[allow(clippy::too_many_arguments)]
// Paragraph construction consumes the span geometry records used by the probes.
#[allow(clippy::type_complexity)]
pub(crate) fn native_paragraph(
    inherited: &BoxStyle,
    opts: &RenderOpts,
    native_request: Option<&NativeRequest<'_>>,
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
    line_atoms: LineAtoms,
) -> AnyElement {
    let native_request = native_request.filter(|_| native);
    let inherited = native_request.map_or(inherited, |request| request.style);
    // Edge-aligned pieces protrude from the block strut (CSS 2.1 §10.8.1).
    // Their size must not enlarge the central inline content's strut.
    // Per-run boxes below retain that strut alongside the legacy fallback.
    let (_, flow_lh) = if edges.is_empty() {
        (biggest, 0.0)
    } else {
        flow_metrics(&pieces, &edges, inherited, opts)
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => gpui::px(v),
        Some(Len::Pct(_) | Len::Em(_)) => {
            gpui::px(inherited.numeric_line_px(opts.base_size()).unwrap_or(0.0))
        }
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
            opts.cx.vcx(),
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
        super::alignment::first(inherited, opts),
        wrap,
    )
    .measure_context(&opts.cx)
    .ruby_context(&opts.cx);
    let para = paragraph_spans(para, inherited, opts, &pieces, &edges, biggest, line)
        .edge_spans(edges)
        .line_boxes(
            boxes.as_ref().map(|b| b.0.clone()).unwrap_or_default(),
            // A turned central line aligns its runs to the root inline box
            // (`Paragraph::shared_base`), so it needs the strut font as well.
            (boxes.is_some() || rotated_central(inherited)).then(|| {
                (
                    inline::strut_font(inherited, &opts.text, opts.cx.vcx()),
                    gpui::px(own_size(inherited, opts)),
                )
            }),
        )
        .rel_spans(rotate_rel_spans(inline::rel_spans(&pieces), inherited))
        .ruby_justify(
            inherited.ruby_justify == Some(true),
            inherited.ruby_unit,
            inherited.wm().turned(),
            line_atoms.iter().any(|atom| !atom.3.is_empty()),
        )
        .justify_chars(inherited.justify_chars.unwrap_or(1))
        .align_last(super::alignment::last(inherited, opts))
        .letter_spacing(gpui::px(metrics::spacing_px(
            inherited.letter_spacing,
            &family,
            biggest,
            opts.cx.vcx(),
        )))
        .word_spacing(gpui::px(metrics::spacing_px(
            inherited.word_spacing,
            &family,
            biggest,
            opts.cx.vcx(),
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
        .line_clamp(clamp_budget.or_else(|| {
            (opts.cx.clamp.input.is_none()
                && !crate::text::clamp::clamp_context(&opts.cx).is_some_and(|(_, skip)| skip)
                && opts.cx.clamp.atom_depth == 0)
                .then(|| inherited.clamp_lines().map(|n| n as usize))
                .flatten()
        }))
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
        .clamp_mark(inherited.used_block_ellipsis())
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
                .ink()
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
        .hyphen_limit_lines(inherited.hyphen_limit_lines)
        .hyphen_char(
            crate::render::paragraph::paragraph_style(inherited)
                .hyphen_char
                .clone(),
        )
        .tab_stops(inline::tab_stops(
            &pieces,
            inherited,
            &opts.text,
            opts.cx.vcx(),
        ))
        .overlays(inline::overlays(pieces))
        .atoms(line_atoms)
        // Срез строки — только у абзаца, держащего первую/последнюю строку
        // срезающего контейнера (`text::line_trim`).
        .ruby_trim(
            crate::text::line_trim::para(opts.cx.inline.trim_para, inherited.self_node).unwrap_or(
                crate::text::line_trim::Trim {
                    start: inherited.text_box_trim_start,
                    end: inherited.text_box_trim_end,
                    over: 0.0,
                },
            ),
        );
    let para = para.fragment_flow(
        opts.frag.watch.as_ref().map(|w| w.flow.clone()),
        crate::layout::fragment::types::StackAxis::of(inherited.wm()),
        (
            usize::from(inherited.orphans.unwrap_or(2)),
            usize::from(inherited.widows.unwrap_or(2)),
        ),
    );
    let para = crate::render::paragraph::first_line_box::configure(para, inherited, opts);
    let para = if selectable {
        para.selectable(id, opts.selection_color())
    } else {
        para
    };
    if let Some(request) = native_request {
        request.built.set(true);
    }
    let projection = crate::interactive::scroll_extent::projection(
        opts.cx.scroll.frame.as_ref(),
        inherited.self_node,
        None,
        opts.cx.scroll.sticky.as_ref(),
    );
    let sticky = projection
        .as_ref()
        .map(|p| p.link().clone())
        .or_else(|| opts.cx.scroll.sticky.clone());
    crate::interactive::scroll_extent::project(
        para.scroll_owner(inherited.self_node, sticky, opts.cx.scroll.frame.clone())
            .into_any_element(),
        projection,
    )
}
