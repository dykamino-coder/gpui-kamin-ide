//! Боковые поля, рамки и отступы строчной коробки в точках при сборе кусков (CSS 2.1 §8.3,
//! §8.4): физические стороны проецируются в плоскость набора абзаца, единицы шрифта
//! разрешаются по слитому стилю, доля — от ширины содержащего блока. Поле возвращается
//! отдельно от рамки с отступом: под полем виден фон предка, под отступом — свой (§14.2).
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Боковые поля, рамки и отступы строчной коробки в точках.
///
/// Доля тут не считается: она берётся от ширины контейнера, которая на сборке
/// кусков ещё не решена. Пропуск честнее приблизительной длины — её видно в
/// сравнении с браузером.
pub(super) fn inline_sides(
    e: &Element,
    merged: &Computed,
    flow: &Computed,
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) -> ((f32, f32), (f32, f32)) {
    let size = match merged.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let family = merged.font_family.clone().unwrap_or_default();
    // Доля поля и отступа строчной коробки — от ширины содержащего блока
    // (CSS 2.1 §8.3, §8.4: «percentage … refer to the width of the
    // containing block»), то есть блока абзаца. Прежде доля молча давала
    // ноль (`text-indent-percentage-001`: эталон `margin-left: 50%` на
    // `<span>` стоял у края). Блок абзаца — `cb` (`LayoutCx::avail_w`).
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(_) | Len::Em(_) | Len::Ch(_) | Len::Ex(_) | Len::Ic(_)) => {
            crate::text::metrics::spacing_px(l, &family, size, vcx)
        }
        Some(Len::Pct(k)) => cb.map_or(0.0, |w| k * w),
        _ => 0.0,
    };
    let border = e.style.cascaded().borders();
    // Боковой отступ строчной коробки занимает место в строке ВСЕГДА (§8.4),
    // фон там задан или нет: `padding-right: 4em` двигает следующее слово и
    // рвёт строку. Прежде отступ гасился при заданном фоне — считалось, что
    // его держит прогон текста (`inline_pad`), но прогон только КРАСИТ:
    // ширина квада приходит разностью положений глифов, а раздутие на
    // `pad[1]`/`pad[3]` продвижения не даёт.
    //
    // Известное исключение — `css-text/shaping-arabic-diacritics-002`: отступ
    // задан спану ВНУТРИ арабского слова, и распорка U+FEFF рвёт курсивное
    // соединение; чинится не здесь, а прозрачностью распорки для набора.
    // Поле возвращается ОТДЕЛЬНО от рамки с отступом: под полем виден фон
    // ПРЕДКА (§8.3 — поля всегда прозрачны), а под рамкой и отступом — свой
    // (§14.2). Одной распоркой обе полосы не выразить: фон у неё один.
    let flat = |s: crate::style::computed::Sides| {
        super::physical_sides::project(flow, [s.top, s.right, s.bottom, s.left]).map(px_of)
    };
    let margin = flat(e.style.cascaded().margin);
    let padding = flat(e.style.cascaded().padding);
    let border = flat(border);
    (
        (margin[3], margin[1]),
        (border[3] + padding[3], border[1] + padding[1]),
    )
}
