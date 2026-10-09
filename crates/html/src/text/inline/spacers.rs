//! Спейсеры инлайн-коробок: стили, рамки, края, наложения.

use crate::text::inline::*;
use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};
use gpui::AnyElement;

/// Слой знака-распорки: ширину ему даёт трекинг на своём куске, а всё
/// остальное с него снимается — фон и замена пробелов принадлежат тексту.
///
/// Фон снимается ИМЕННО ЗДЕСЬ, а поле красится своей распоркой
/// (`margin_spacer_style`): под полем виден фон предка, под отступом — свой.
/// Первый заход на это без правки набора дал 0 и 0 на 3646 парах: распорка
/// фон получала, но полоса не рисовалась. Корень был в наборе — «default
/// ignorable» U+FEFF выбрасывается целиком, глифов у прогона не остаётся, и
/// цикл краски полосы не идёт ни разу. Чинится в `vendor/gpui`
/// (`line_layout.rs` — ширина по знакам, `line.rs` — квад безглифного
/// прогона), обе пометки «KaminIDE patch».
/// Borderless padding paints on its edge spacer independently of a descendant's
/// background (CSS 2.1 sections 8.4 and 14.2), with no duplicate band extension.
/// Empty inline boxes still use their explicit overlay for the vertical sides.
///
pub(super) fn spacer_style(merged: &Computed, advance: f32) -> Computed {
    let mut style = merged.clone();
    style.letter_spacing = Some(Len::Px(advance));
    style.word_spacing = None;
    style.word_space_char = None;
    style.inline_bg = None;
    style.inline_border = None;
    style
}

pub(super) fn padding_spacer_style(merged: &Computed, advance: f32, painted: bool) -> Computed {
    let mut style = spacer_style(merged, advance);
    if painted {
        style.inline_bg = merged.inline_bg;
    }
    style
}

/// Слой распорки ПОЛЯ строчной коробки: своего фона у поля нет, сквозь него
/// виден фон предка (§8.3 «margin properties … are always transparent»).
/// Рамку распорке поля не даём: полосу с рамкой уже мерили дважды, обе потери
/// в `bidi-*` (см. запись у `uniform_border`).
pub(super) fn margin_spacer_style(merged: &Computed, inherited: &Computed, advance: f32) -> Computed {
    let mut style = spacer_style(merged, advance);
    style.inline_bg = inherited.inline_bg;
    style
}

/// Ровная рамка строчной коробки: одинаковые цвет и толщина у всех граней.
///
/// Только такую умеет нарисовать прогон текста. Разные грани оставляем
/// коробке в раскладке — там они честные, но текст в ней не переносится
/// вместе с абзацем.
/// Рамка с РАЗНЫМИ гранями, выражаемая прогоном: все заданные стороны в
/// точках и ЕДИНЫЙ цвет (или цвет текста). Возврат — [верх, право, низ,
/// лево]; незаданные стороны нулевые.
/// Толщина грани строчной рамки в точках.
///
/// Шрифтовые единицы разрешаются по СВОЕМУ кеглю — тем же `spacing_px`, каким
/// уже считаются боковые поля и отступы строчной коробки (`inline_sides`).
/// Прежде сюда пускались только `Len::Px`, и `border-left: 0.2em` — самая
/// обычная запись — уводила `<span>` из прогона текста в настоящую коробку:
/// она садится в строку атомом и растит строку на спуск шрифта. По
/// css-backgrounds-3 §4.1 `<line-width>` — это `<length [0,∞]>` любых единиц,
/// сужения до точек спека не даёт.
fn border_px(l: Option<Len>, c: &Computed, font_px: f32) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(v),
        Some(u @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
            // Кегль берётся у ВЫЗЫВАЮЩЕГО: сюда приходит собственный стиль
            // элемента, а `font-size` у `<span>` чаще всего не объявлен —
            // единицы считаются по слитому кеглю строки.
            let size = match c.font_size {
                Some(Len::Px(v)) => v,
                _ => font_px,
            };
            let family = c.font_family.clone().unwrap_or_default();
            Some(crate::text::metrics::spacing_px(Some(u), &family, size))
        }
        _ => None,
    }
}

pub fn sided_border(c: &Computed, font_px: f32) -> Option<(Color, [f32; 4])> {
    let w = c.borders();
    let px_of = |l: Option<Len>| border_px(l, c, font_px);
    let sides = [
        px_of(w.top)?,
        px_of(w.right)?,
        px_of(w.bottom)?,
        px_of(w.left)?,
    ];
    if !sides.iter().any(|v| *v > 0.0) {
        return None;
    }
    // Цвет сверяется только по ЗАДАННЫМ сторонам: у частичной рамки
    // остальных цветов просто нет.
    let mut color: Option<Color> = None;
    for i in 0..4 {
        if sides[i] <= 0.0 {
            continue;
        }
        // Цвет рамки по умолчанию — цвет текста, а он на строчном куске
        // часто не задан вовсе: без запасного чёрного прогон отказывался от
        // рамки, и кусок уходил в коробку, двигая текст на её ширину. То же
        // умолчание уже стоит на блочном пути (`apply.rs`).
        //
        // ЗАМЕРЕНО ОТДЕЛЬНО И ОТКАЧЕНО ДВАЖДЫ: включить в `vendor/gpui` полосу
        // прогона для рамки БЕЗ фона (сейчас квад заводится только при
        // заданной подсветке, и рамка строчной коробки не рисуется вовсе).
        // Первый замер 19/17, второй — вместе со сверкой `background_border`
        // при слиянии прогонов (`text_system.rs`) — 0 приобретено, 17
        // потеряно, все потери в семьях `bidi-*`. Разбор в
        // `target/scout-band.md`: полоса открывается и закрывается внутри
        // КАЖДОГО визуального прогона, поэтому на стыке двунаправленности и
        // на переносе рисуются обе боковые грани. Возвращать вместе с
        // признаками «полоса продолжается» у `pad_left`/`pad_right`.
        let side = c.border_colors[i]
            .or(c.border_color)
            .or(c.color)
            .unwrap_or(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            });
        match color {
            None => color = Some(side),
            Some(prev) if prev == side => {}
            _ => return None,
        }
    }
    Some((color?, sides))
}

pub fn uniform_border(c: &Computed, font_px: f32) -> Option<(Color, f32)> {
    let w = c.borders();
    // Здесь незаданная грань — НЕ ноль: ровной рамке нужны все четыре, и
    // `None` обязан рушить сведение (иначе `border-left` в одиночку сошёл бы
    // за ровную рамку по всем сторонам).
    let px_of = |l: Option<Len>| l.and_then(|v| border_px(Some(v), c, font_px));
    let (t, r, b, l) = (
        px_of(w.top)?,
        px_of(w.right)?,
        px_of(w.bottom)?,
        px_of(w.left)?,
    );
    if t <= 0.0 || t != r || t != b || t != l {
        return None;
    }
    let sides = &c.border_colors;
    let color = match (c.border_color, sides[0], sides[1], sides[2], sides[3]) {
        (_, Some(a), Some(b2), Some(c2), Some(d)) if a == b2 && a == c2 && a == d => a,
        (Some(one), None, None, None, None) => one,
        // Цвет не задан вовсе — рамка красится цветом текста, а без него
        // чёрным (то же умолчание, что у блочного пути).
        (None, None, None, None, None) => c.color.unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }),
        _ => return None,
    };
    Some((color, t))
}

/// Знак-распорка: место под поля, рамки и отступы СТРОЧНОЙ коробки.
///
/// Своей коробки в раскладке у неё нет, поэтому ширину даёт трекинг на этом
/// знаке. Соединитель слов (U+FEFF) взят за то, что точкой переноса он не
/// является: строка не должна рваться по краю `<span>`. Но по UAX-14 его класс
/// (WJ) запрещает разрыв и ПЕРЕД собой — а значит, и по пробелу перед коробкой.
/// Поэтому точки переноса считаются по тексту БЕЗ распорок (`Paragraph`).
pub const SPACER: &str = "\u{feff}";

/// Метка атомарного куска: ширины не несёт, ряд пробелов не рвёт.
pub const ZWSP: &str = "\u{200b}";

/// Места распорок в тексте абзаца — байтовые смещения.
///
/// Считаются по тем же правилам, что и `text_and_runs`. Распорка — всегда
/// СВОЙ кусок ровно из одного знака: так она и отличается от того же знака,
/// пришедшего из документа.
/// Box ids of inline edge spacers (`Computed::spacer_edge`); 0 means none.
pub(super) static SPACER_BOX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Ids for the transparent band colour of inline boxes with a border and no
/// background (see `collect_with_empty_metrics`).
pub(super) static BORDER_BAND: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Edge spacers with their box: (byte offset, box id, physical left edge,
/// parent rtl), in logical order.
pub fn spacer_edges(pieces: &[Piece]) -> Vec<(usize, u32, bool, bool)> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text == SPACER
            && let Some((id, left, parent_rtl)) = style.spacer_edge
        {
            out.push((at, id, left, parent_rtl));
        }
        at += text.len();
    }
    out
}

/// Content extents `(box id, start, end)` of inline boxes with edge spacers,
/// from the zero-length markers around their content.
pub fn box_extents(pieces: &[Piece]) -> Vec<(u32, usize, usize)> {
    let mut at = 0usize;
    let mut out: Vec<(u32, usize, usize)> = Vec::new();
    for p in pieces {
        let Piece::Text { text, style } = p else {
            continue;
        };
        if text.is_empty()
            && let Some((id, start, _)) = style.spacer_edge
        {
            if start {
                out.push((id, at, usize::MAX));
            } else if let Some(b) = out.iter_mut().rev().find(|b| b.0 == id) {
                b.2 = at;
            }
        }
        at += text.len();
    }
    out.retain(|b| b.2 != usize::MAX);
    out
}

pub fn spacers(pieces: &[Piece]) -> Vec<usize> {
    let mut at = 0usize;
    let mut out = Vec::new();
    for p in pieces {
        let Piece::Text { text, .. } = p else {
            continue;
        };
        if text == SPACER {
            out.push(at);
        }
        at += text.len();
    }
    out
}

/// Куски ВНЕ потока и их место в тексте абзаца — байтовое смещение.
///
/// Считается по тем же правилам, что и `text_and_runs`: смещение равно длине
/// текста, собранного до этого куска.
pub fn overlays(pieces: Vec<Piece>) -> Vec<(usize, AnyElement, OverlayAt)> {
    let mut at = 0usize;
    let mut out = Vec::new();
    // Метки краёв строчных содержащих блоков: `id -> (начало, конец)`.
    let mut marks: Vec<(u32, usize, usize)> = Vec::new();
    for p in pieces {
        match p {
            Piece::Text { text, .. } => at += text.len(),
            Piece::Overlay(_, OverlayAt { cb_marker: Some((id, true)), .. }) => {
                marks.push((id, at, at));
            }
            Piece::Overlay(_, OverlayAt { cb_marker: Some((id, false)), .. }) => {
                if let Some(m) = marks.iter_mut().find(|m| m.0 == id) {
                    m.2 = at;
                }
            }
            Piece::Overlay(el, how) => out.push((at, el, how)),
            Piece::Atom(_) => {}
        }
    }
    for (at, _, how) in out.iter_mut() {
        if let Some(cb) = how.cb.as_mut() {
            match marks.iter().find(|m| m.0 == cb.id) {
                Some(&(_, s, e)) => {
                    cb.start = s as isize - *at as isize;
                    cb.end = e as isize - *at as isize;
                }
                None => how.cb = None,
            }
        }
    }
    out
}
