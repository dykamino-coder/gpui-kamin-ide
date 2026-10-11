//! Текст и прогоны абзаца, строка из элементов, склейка атомов.

mod atom_glue;
mod wrapped_row;
use crate::text::inline::runs::atom_glue::glue_atoms;
pub use crate::text::inline::runs::wrapped_row::as_wrapped_row;

use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};
use crate::text::inline::*;
use gpui::{FontStyle, FontWeight, HighlightStyle, TextRun, TextStyle, UnderlineStyle};

/// Можно ли собрать всё в один текстовый блок: одинаковый размер шрифта и ни
/// одного не-текстового куска.
/// ПРОБОВАЛИ И ОТКАТИЛИ: пускать абзац с атомом нормальным путём строк.
/// Собран весь первый шаг разбора (`target/scout-atomstep1.md`): `Piece::Atom`
/// со своей коробкой полей, кусок-НОСИТЕЛЬ `U+FEFF` с трекингом рядом с ним,
/// `atom_spans`/`atom_marks`, рост строки в `line_padding`, подмена носителя
/// на `U+FFFC` в `linebreaks` (css-text-3 §5.1), накопленный шаг строк и
/// посадка атома нижним краем на базовую линию в `point_of`, узкие ворота
/// `atoms_ready`. Замерено по обоим сводам: CSS2 5234 -> 5210, CSS3
/// 2352 -> 2342; приобретено 3, потеряно 38.
///
/// Что вскрылось: (1) базовая линия `inline-block` — нижний край коробки полей
/// ТОЛЬКО при `overflow` не `visible`, иначе это базовая линия последней
/// строки содержимого (§10.8.1), а его высота на сборке неизвестна;
/// (2) доли подъёма и спуска струта 0.8/0.2 верны для Ahem, но не для
/// шрифтов корпуса; (3) накопленный шаг в `point_of` сам по себе роняет всё,
/// что кладётся слоем поверх строки (`text-combine-upright-*`,
/// `position-relative-table-*-absolute-child`) — эту часть надо мерить
/// отдельно и чинить вместе с `paint`.
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
pub fn max_font_size(pieces: &[Piece], strut: f32, em_base: f32) -> f32 {
    pieces.iter().fold(strut, |acc, p| match p {
        Piece::Text { style, .. } => match style.font_size {
            Some(Len::Px(v)) => acc.max(v),
            Some(Len::Em(k)) => acc.max(k * em_base),
            _ => acc,
        },
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
pub fn max_line_height(pieces: &[Piece], strut: f32, em_base: f32, fraction: f32) -> f32 {
    pieces.iter().fold(strut * fraction, |acc, p| match p {
        Piece::Text { style, .. } => {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * em_base,
                _ => strut,
            };
            let own = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => size * fraction,
            };
            acc.max(own)
        }
        Piece::Atom(_) | Piece::Overlay(..) => acc,
    })
}

/// Один `StyledText` с прогонами — честный перенос по словам сквозь границы
/// `<b>`/`<a>`/`<span>`.
/// Текст и прогоны абзаца — то же, что уходит в `StyledText`.
///
/// Отдаются отдельно, потому что выделение строит свой элемент из тех же
/// прогонов, добавляя подложку на выделенный кусок.
pub fn text_and_runs(pieces: &[Piece], base: &TextStyle) -> Option<(String, Vec<TextRun>)> {
    let mut text = String::new();
    let mut runs: Vec<TextRun> = vec![];
    for p in pieces {
        match p {
            Piece::Text { text: t, style } => {
                if t.is_empty() {
                    continue;
                }
                text.push_str(t);
                runs.push(run_for(t, style, base));
            }
            // Кусок вне потока в текст не входит: его место помечает нулевой
            // пробел, а сам он рисуется поверх (см. `overlays`).
            Piece::Overlay(..) => {}
            Piece::Atom(_) => return None,
        }
    }
    (!text.is_empty()).then_some((text, runs))
}

/// Шрифт струта абзаца — тот же, каким набирался бы текст самого блока.
pub fn strut_font(style: &Computed, base: &TextStyle) -> gpui::Font {
    run_for("x", style, base).font
}

pub(super) fn run_for(text: &str, style: &Computed, base: &TextStyle) -> TextRun {
    let mut font = base.font();
    font.fallbacks = crate::style::computed::font_family::fallbacks(style, font.fallbacks);
    // Названное семейство сильнее родового: подстановкой занимается система.
    // Пустое имя — «шрифт документа» (разбор `font-family`): база как есть.
    if let Some(family) = style.font_family.as_ref().filter(|f| !f.is_empty()) {
        // Имя из разметки может быть придуманным (`@font-face`) — система
        // шрифтов знает файл под его собственным именем. Лиц у имени бывает
        // несколько, и нужное выбирает ширина начертания (§font-matching).
        font.family = crate::text::fonts::alias_stretch(family, style.font_stretch)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if style.monospace == Some(true) {
        font.family = crate::text::metrics::mono_family_for(style.lang.as_deref()).into();
    }
    // Вес и курсив — ВСЕГДА от стиля куска: `None` в слитом стиле — это
    // обычное начертание, а не «как у базы» (база абзаца строится по
    // первому куску, и `abc<b>def</b>ghi` набирался одним прогоном его
    // веса — сквозной дефект по всему корпусу).
    font.weight = FontWeight(style.font_weight.unwrap_or(400) as f32);
    font.style = if style.italic == Some(true) {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    };
    if let Some(pct) = style.font_stretch {
        font.stretch = gpui::FontStretch::from_percent(pct);
    }
    let features = style.used_features();
    if !features.is_empty() {
        font.features = gpui::FontFeatures(std::sync::Arc::new(features));
    }
    // `visibility: hidden` на самом куске: место в строке он держит, а чернил
    // не даёт (§11.2). Прозрачный цвет, а не пропуск куска, — иначе поехали бы
    // ширины и переносы.
    let color = if style.hidden == Some(true) {
        gpui::hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        style.color.map(Color::to_hsla).unwrap_or(base.color)
    };
    // Кегль куска: без него разный размер в строке собрать в один блок было
    // нельзя (см. `single_block`).
    let base_px = f32::from(base.font_size.to_pixels(gpui::px(16.)));
    let font_size = match style.font_size {
        Some(Len::Px(v)) => Some(gpui::px(v)),
        Some(Len::Em(k)) => Some(gpui::px(k * base_px)),
        _ => None,
    };
    TextRun {
        len: text.len(),
        font,
        font_size,
        color,
        // A fully transparent band colour only identifies its inline box (see
        // `BORDER_BAND`): it is kept black (lightness 0) so nothing of it can
        // blend into the border edge, with the id in hue and saturation.
        background_color: style.inline_bg.map(|c| {
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
        background_border: style
            .inline_border
            .map(|(c, w)| (c.to_hsla(), w.map(gpui::px))),
        background_pad: style.inline_pad.unwrap_or_default().map(gpui::px),
        background_radius: gpui::px(style.inline_radius.unwrap_or(0.0)),
        underline: style.underline.unwrap_or(false).then(|| UnderlineStyle {
            thickness: gpui::px(1.),
            color: Some(color),
            wavy: false,
        }),
        strikethrough: style
            .line_through
            .unwrap_or(false)
            .then(|| gpui::StrikethroughStyle {
                thickness: gpui::px(1.),
                color: Some(color),
            }),
    }
}

/// Подсветка для куска текста — используется, когда прогоны накладываются на
/// готовый текст (например, при подсветке кода).
pub fn highlight_for(style: &Computed) -> HighlightStyle {
    HighlightStyle {
        color: style.color.map(Color::to_hsla),
        font_weight: style.font_weight.map(|w| FontWeight(w as f32)),
        font_style: style.italic.and_then(|i| i.then_some(FontStyle::Italic)),
        ..Default::default()
    }
}
