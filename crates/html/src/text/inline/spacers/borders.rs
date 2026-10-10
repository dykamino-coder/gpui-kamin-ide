//! Borders for spacers; split out to keep the owning module within 250 lines.

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
pub(super) fn border_px(l: Option<Len>, c: &Computed, font_px: f32) -> Option<f32> {
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
