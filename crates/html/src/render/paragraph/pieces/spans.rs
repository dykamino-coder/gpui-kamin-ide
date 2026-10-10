//! Направление и интервалы текста при настройке абзаца.

use crate::layout::writing_mode::native_vertical;
use crate::render::*;
use crate::style::computed::Computed;
use crate::text::inline;
use gpui::px;

#[allow(clippy::too_many_arguments)]
pub(crate) fn paragraph_spans(
    para: crate::text::paragraph::Paragraph,
    inherited: &Computed,
    opts: &RenderOpts,
    pieces: &[inline::Piece],
    edges: &[(std::ops::Range<usize>, bool, f32)],
    biggest: f32,
    line: gpui::Pixels,
) -> crate::text::paragraph::Paragraph {
    para
        // Preserve wrapping, direction and the sideways alphabetic baseline.
        .opaque_background(inherited)
        .reversed_lines(
            crate::render::paragraph::paragraph_style(inherited).lines_reversed == Some(true),
        )
        .vertical(
            inherited.para_vertical.is_some(),
            inherited.para_vertical == Some(true),
        )
        .ortho_limit(inherited.ortho_limit.map(px))
        .vertical_central_baseline(
            inherited.sideways != Some(true) && inherited.text_sideways != Some(true),
        )
        .rotated_central(
            inherited.rotated_line == Some(true)
                && inherited.sideways != Some(true)
                && inherited.text_sideways != Some(true),
        )
        .vertical_counter_clockwise(
            inherited.para_vertical == Some(false) && inherited.sideways == Some(true),
        )
        .vertical_inline_constraint(
            inherited.orthogonal_inline,
            native_vertical::keyword(inherited),
        )
        .plaintext(
            inherited
                .bidi_plaintext
                .unwrap_or(false)
                .then(|| {
                    inherited
                        .text_align
                        .unwrap_or(crate::style::computed::TextAlign::Start)
                })
                .filter(|a| {
                    matches!(
                        a,
                        crate::style::computed::TextAlign::Start
                            | crate::style::computed::TextAlign::End
                    )
                }),
        )
        .spans(inline::wrap_spans(pieces, inherited))
        .word_spans(inline::word_spans(pieces, biggest))
        // Автозазоры идут ПЕРВЫМИ: поиск диапазона берёт первое
        // попадание, и зазор обязан перебить трекинг всего куска.
        .letter_spans(
            [
                inline::autospace_spans(pieces, biggest),
                inline::letter_spans(pieces, biggest),
            ]
            .concat(),
        )
        .shift_spans({
            let mut v = inline::shift_spans(pieces, biggest, f32::from(line));
            v.retain(|(r, _)| !in_edge(edges, r));
            v
        })
        .lh_spans({
            let mut v = inline::line_height_spans(
                pieces,
                inherited,
                biggest,
                crate::text::metrics::normal_line(
                    &inherited.font_family.clone().unwrap_or_default(),
                ),
            );
            v.retain(|(r, _)| !in_edge(edges, r));
            v
        })
        // В повёрнутом абзаце руби в строку не идёт и строку не растит
        // (`atoms_fit_line`), а эталоны акцента сделаны из руби: рост
        // только у горизонтального (`text-emphasis-line-height-003*/004*`).
        .emph_spans(
            if inherited.rotated_line == Some(true) || inherited.vertical == Some(true) {
                Vec::new()
            } else {
                inline::emphasis_spans(
                    pieces,
                    biggest,
                    crate::text::metrics::normal_line(
                        &inherited.font_family.clone().unwrap_or_default(),
                    ),
                )
            },
        )
        // Линии украшений рисует сам абзац (css-text-decor-3 §2); у
        // повёрнутого — прежний путь набора.
        .decor_spans(inline::decor_spans(pieces, &opts.text))
}
