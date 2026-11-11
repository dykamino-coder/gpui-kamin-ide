//! Текст и прогоны абзаца, строка из элементов, склейка атомов.

use crate::defaults::MEDIUM_FONT_SIZE_PX;
mod atom_glue;
mod capacity;
mod face;
mod wrapped_row;
use crate::text::inline::runs::atom_glue::glue_atoms;
pub use crate::text::inline::runs::wrapped_row::as_wrapped_row;

use crate::layout::cx::run_faces::RunFaces;
use crate::style::computed::Computed;
use crate::style::values::value::Color;
use crate::text::inline::*;
use gpui::{TextRun, TextStyle, UnderlineStyle};

/// Можно ли собрать всё в один текстовый блок: одинаковый размер шрифта и ни
/// одного не-текстового куска.
///
/// Пускать абзац с атомом нормальным путём строк (`Piece::Atom` со своей
/// коробкой полей, кусок-носитель `U+FEFF` с трекингом, `atom_spans`/
/// `atom_marks`, рост строки в `line_padding`, подмена носителя на `U+FFFC` в
/// `linebreaks` по css-text-3 §5.1, посадка атома на базовую линию в
/// `point_of`, ворота `atoms_ready`) пока нельзя: (1) базовая линия
/// `inline-block` — нижний край коробки полей ТОЛЬКО при `overflow` не
/// `visible`, иначе это базовая линия последней строки содержимого (§10.8.1),
/// а его высота на сборке неизвестна; (2) доли подъёма и спуска струта
/// 0.8/0.2 верны для Ahem, но не для шрифтов корпуса; (3) накопленный шаг в
/// `point_of` роняет всё, что кладётся слоем поверх строки
/// (`text-combine-upright-*`, `position-relative-table-*-absolute-child`) —
/// эту часть чинить вместе с `paint`.
pub fn single_block(pieces: &[Piece], _base_size: f32) -> bool {
    // Кегль куску больше не мешает: он едет в прогон (патч GPUI). Мешает
    // только не-текстовый кусок — картинку в прогон не положить. Кусок ВНЕ
    // потока не мешает: место в строке он не занимает.
    pieces
        .iter()
        .all(|p| matches!(p, Piece::Text { .. } | Piece::Overlay(..)))
}

/// Самый крупный кегль среди кусков — по нему считается высота строки.
/// `strut` — кегль самого абзаца: строка не бывает ниже его. `em_base` —
/// от чего считается доля `em` у куска: это кегль РОДИТЕЛЯ абзаца, а не его
/// собственный, иначе `font-size: 4em` на абзаце и на его куске умножились бы
/// дважды (`text-transform-shaping-001`: строка вышла в 256 точек вместо 64).
pub fn max_font_size(
    pieces: &[Piece],
    strut: f32,
    em_base: f32,
    vcx: crate::values::ValueCx<'_>,
) -> f32 {
    pieces.iter().fold(strut, |acc, p| match p {
        Piece::Text { style, .. } => acc.max(style.used_font(em_base, vcx).size_px),
        Piece::Atom(_) | Piece::Overlay(..) => acc,
    })
}

/// Высота строки абзаца: максимум по кускам, но с УЧЁТОМ объявленной на
/// куске `line-height`.
///
/// Прежде высота считалась как «самый крупный кегль × доля normal», и
/// объявленная на куске `line-height` доходила только каналом `lh_spans`,
/// который умеет строку растить и не умеет сжимать. У `font: 100px/1` строка
/// выходила 132 вместо 100, а глиф садился по полулидингу на 16 точек ниже
/// (§10.8: лидинг тут ноль — содержимое равно `line-height`).
pub fn max_line_height(
    pieces: &[Piece],
    strut: f32,
    em_base: f32,
    fraction: f32,
    fonts: &crate::text::text_fonts::TextFonts,
    vcx: crate::values::ValueCx<'_>,
) -> f32 {
    pieces.iter().fold(
        crate::text::metrics::normal_px(fraction, strut, fonts),
        |acc, p| match p {
            Piece::Text { style, .. } => {
                let size = style.used_font(em_base, vcx).size_px;
                let own = style
                    .numeric_line_px(em_base)
                    .unwrap_or_else(|| crate::text::metrics::normal_px(fraction, size, fonts));
                acc.max(own)
            }
            Piece::Atom(_) | Piece::Overlay(..) => acc,
        },
    )
}

/// Один `StyledText` с прогонами — честный перенос по словам сквозь границы
/// `<b>`/`<a>`/`<span>`.
/// Текст и прогоны абзаца — то же, что уходит в `StyledText`.
///
/// Отдаются отдельно, потому что выделение строит свой элемент из тех же
/// прогонов, добавляя подложку на выделенный кусок.
pub fn text_and_runs(
    pieces: &[Piece],
    base: &TextStyle,
    vcx: crate::values::ValueCx<'_>,
    faces: Option<&RunFaces>,
) -> Option<(String, Vec<TextRun>)> {
    let (bytes, count) = capacity::sizes(pieces)?;
    let (mut text, mut runs) = (String::with_capacity(bytes), Vec::with_capacity(count));
    for p in pieces {
        match p {
            Piece::Text { text: t, style } => {
                if t.is_empty() {
                    continue;
                }
                text.push_str(t);
                runs.push(decorated_run(
                    t,
                    style,
                    style.inline_decoration,
                    style.ink(),
                    base,
                    vcx,
                    faces,
                ));
            }
            // Кусок вне потока в текст не входит: его место помечает нулевой
            // пробел, а сам он рисуется поверх (см. `overlays`).
            Piece::Overlay(..) | Piece::Atom(_) => {}
        }
    }
    // OOF-only paragraphs retain a static point without creating a line.
    (!text.is_empty() || pieces.iter().any(|p| matches!(p, Piece::Overlay(..))))
        .then_some((text, runs))
}

/// Шрифт струта абзаца — тот же, каким набирался бы текст самого блока.
pub fn strut_font(
    style: &Computed,
    base: &TextStyle,
    vcx: crate::values::ValueCx<'_>,
    faces: Option<&RunFaces>,
) -> gpui::Font {
    let base_px = f32::from(base.font_size.to_pixels(gpui::px(MEDIUM_FONT_SIZE_PX)));
    face::run_face(style, base, base_px, vcx, faces).0
}

pub(super) fn run_for(
    text: &str,
    style: &Computed,
    base: &TextStyle,
    vcx: crate::values::ValueCx<'_>,
) -> TextRun {
    let ink = style.inh.ui.color;
    decorated_run(text, style, Default::default(), ink, base, vcx, None)
}

fn decorated_run(
    text: &str,
    style: &Computed,
    inline_decoration: crate::style::inline_decoration::InlineDecoration,
    ink: Option<Color>,
    base: &TextStyle,
    vcx: crate::values::ValueCx<'_>,
    faces: Option<&RunFaces>,
) -> TextRun {
    let decoration = &style.reset_text_box;
    let base_px = f32::from(base.font_size.to_pixels(gpui::px(MEDIUM_FONT_SIZE_PX)));
    let (font, font_size) = face::run_face(style, base, base_px, vcx, faces);
    // `visibility: hidden` на самом куске: место в строке он держит, а чернил
    // не даёт (§11.2). Прозрачный цвет, а не пропуск куска, — иначе поехали бы
    // ширины и переносы.
    let color = if style.inh.ui.hidden == Some(true) {
        gpui::hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        decoration
            .text_shadow_ink
            .or(ink)
            .map(Color::to_hsla)
            .unwrap_or(base.color)
    };
    TextRun {
        len: text.len(),
        font,
        font_size,
        color,
        // A fully transparent band colour only identifies its inline box (see
        // `BORDER_BAND`): it is kept black (lightness 0) so nothing of it can
        // blend into the border edge, with the id in hue and saturation.
        background_color: inline_decoration.band_color.map(|c| {
            if c.a == 0.0 {
                gpui::Hsla {
                    h: c.r,
                    s: c.g,
                    l: 0.0,
                    a: 0.0,
                }
            } else {
                c.to_hsla()
            }
        }),
        background_border: inline_decoration
            .border_stroke
            .map(|(c, w)| (c.to_hsla(), w.map(gpui::px))),
        background_pad: inline_decoration
            .physical_padding
            .unwrap_or_default()
            .map(gpui::px),
        background_radius: gpui::px(inline_decoration.corner_radius.unwrap_or(0.0)),
        background_line_gap: true,
        underline: decoration
            .underline
            .unwrap_or(false)
            .then(|| UnderlineStyle {
                thickness: gpui::px(1.),
                color: Some(color),
                wavy: false,
            }),
        strikethrough: decoration
            .line_through
            .unwrap_or(false)
            .then(|| gpui::StrikethroughStyle {
                thickness: gpui::px(1.),
                color: Some(color),
            }),
    }
}
