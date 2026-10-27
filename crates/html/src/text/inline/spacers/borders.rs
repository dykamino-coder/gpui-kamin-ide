//! Рамка строчной коробки, выражаемая прогоном текста: ровная (одинаковые цвет и толщина всех
//! граней) или с разными гранями одного цвета, толщина в любых единицах `<line-width>`
//! (css-backgrounds-3 §4.1) по кеглю куска, цвет по умолчанию — цвет текста, без него чёрный.
//! `None` — рамку прогоном не выразить, коробка уходит в раскладку.

use crate::style::computed::Computed;
use crate::style::values::value::{Color, Len};

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
pub(super) fn border_px(
    l: Option<Len>,
    c: &Computed,
    font_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<f32> {
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
            Some(crate::text::metrics::spacing_px(
                Some(u),
                &family,
                size,
                vcx,
            ))
        }
        _ => None,
    }
}

// A CSS edge index selects the corresponding width, color and style entries.
pub fn sided_border(
    c: &Computed,
    font_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Color, [f32; 4])> {
    sided_border_with_colors(
        c,
        std::array::from_fn(|i| c.used_border_color(i)),
        font_px,
        vcx,
    )
}

/// Геометрия читает CSS, а цвета — явный результат своей коробки.
pub(crate) fn sided_border_with_colors(
    c: &Computed,
    colors: [Color; 4],
    font_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Color, [f32; 4])> {
    let w = c.borders();
    let px_of = |l: Option<Len>| border_px(l, c, font_px, vcx);
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
        // умолчание уже стоит на блочном пути (`paint/apply/`).
        //
        // Полосу от рамки БЕЗ фона открывает `box_paint` прозрачным цветом
        // подложки, а не набор в `vendor/gpui` (квад там открывается только от
        // подсветки). Заводить её в наборе от любой рамки нельзя: полоса
        // открывается и закрывается внутри КАЖДОГО визуального прогона, и на
        // стыке двунаправленности и на переносе рисуются обе боковые грани
        // (`bidi-*`), пока у `pad_left`/`pad_right` нет признака «полоса
        // продолжается».
        let side = colors[i];
        match color {
            None => color = Some(side),
            Some(prev) if prev == side => {}
            _ => return None,
        }
    }
    Some((color?, sides))
}

pub fn uniform_border(
    c: &Computed,
    font_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Color, f32)> {
    uniform_border_with_colors(
        c,
        std::array::from_fn(|i| c.used_border_color(i)),
        font_px,
        vcx,
    )
}

/// Ровная рамка с явно переданными цветами краски.
pub(crate) fn uniform_border_with_colors(
    c: &Computed,
    colors: [Color; 4],
    font_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Color, f32)> {
    let w = c.borders();
    // Здесь незаданная грань — НЕ ноль: ровной рамке нужны все четыре, и
    // `None` обязан рушить сведение (иначе `border-left` в одиночку сошёл бы
    // за ровную рамку по всем сторонам).
    let px_of = |l: Option<Len>| l.and_then(|v| border_px(Some(v), c, font_px, vcx));
    let (t, r, b, l) = (
        px_of(w.top)?,
        px_of(w.right)?,
        px_of(w.bottom)?,
        px_of(w.left)?,
    );
    if t <= 0.0 || t != r || t != b || t != l {
        return None;
    }
    let color = colors[0];
    colors
        .iter()
        .all(|side| *side == color)
        .then_some((color, t))
}
