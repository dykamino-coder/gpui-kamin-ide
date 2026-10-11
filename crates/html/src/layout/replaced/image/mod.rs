//! Картинки.
// owner: A

use crate::dom::Element;
use crate::render::styled_div;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, div, px};
mod with;
pub(crate) use with::image_with;
mod local;
use local::{fit_local_image, local_image_source};
mod contained;
use contained::contained_image;
mod sized;
use sized::sized_image;
mod auto_sized;
use auto_sized::auto_sized_image;

pub(crate) fn image(e: &Element) -> AnyElement {
    image_with(e, None)
}

/// Доля ВЫСОТЫ замещаемого, сведённая к точкам по содержащему блоку.
///
/// Держатель картинки стоит внутри анонимного ряда строки
/// (`inline::as_wrapped_row`): у ряда высота `auto`, элемент прижат по базовой
/// линии и не растягивается, поэтому доля высоты бралась ОТ РЯДА и разрешалась
/// в ноль — картинка не рисовалась ни одной точкой
/// (`background-image-cover-002-ref`). Содержащий блок здесь известен точно —
/// это `inherited`, и доля сводится к точкам ещё на сборке.
///
/// Только обычный блочный контейнер: у гибкого, сеточного и лунок высота
/// приходит от раскладки, и подстановка ломает `row-auto-repeat-auto-023`.
pub(crate) fn pct_height_to_px(e: &Element, inherited: &Computed) -> Element {
    let e = &pct_limits_to_px(e, inherited);
    let (Some(Len::Pct(k)), Some(Len::Px(h))) = (e.style.height, inherited.height) else {
        return e.clone();
    };
    // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда и `inline-block` — 0 и 0 по обоим
    // сводам, `sizing-percentages-replaced-orthogonal-001` не сдвинулась.
    // Пущен снова (03.10): картинка СТРОКИ внутри `inline-block` с высотой
    // в точках рисовалась природным размером — доля высоты до раскладки
    // не доезжала (`intrinsic-percent-replaced-021`: `<button>` 100px,
    // картинка 200×200 вместо 100×100; проба: тот же `div` блоком — верно).
    if !matches!(
        inherited.display,
        None | Some(Display::Block) | Some(Display::InlineBlock)
    ) {
        return e.clone();
    }
    // `height` в разборе — высота СОДЕРЖИМОГО (§10.6.2, content-box): отступы
    // и рамку прибавляет уже раскладка. Вычитать их отсюда нельзя — картинка
    // выходила на 6 точек короче (0.91 вместо 0.00 на `cover-002`).
    if h <= 0.0 {
        return e.clone();
    }
    // …но при `box-sizing: border-box` у РОДИТЕЛЯ заданная высота включает
    // его отступы и рамку, а опора доли — содержимое (CSS 2.1 §10.5,
    // css-sizing-3 §box-sizing): `.inner {box-sizing: border-box; height: 50px;
    // padding-top: 25px}` даёт холсту 25, а не 50
    // (`intrinsic-percent-replaced-012/013`).
    let h = if inherited.border_box == Some(true) {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = inherited.borders();
        (h - px_of(inherited.padding.top)
            - px_of(inherited.padding.bottom)
            - px_of(b.top)
            - px_of(b.bottom))
        .max(0.0)
    } else {
        h
    };
    let mut copy = e.clone();
    copy.style.height = Some(Len::Px(k * h));
    copy
}

/// Пределы замещаемого в ДОЛЯХ — в точки от содержащего блока.
///
/// Размер рисунка считается на сборке (`image_with`), и доля там уже не с чем
/// сравнивать: `max-width: 100%` у картинки отбрасывался целиком, и она
/// вылезала за родителя (css-sizing-3 §5: пределы решаются от содержащего
/// блока, как и сам размер). Ширина берётся от ширины содержащего блока,
/// высота — от его высоты и только у обычного блочного контейнера: у гибкого
/// и сеточного высота приходит от раскладки (та же оговорка, что у
/// `pct_height_to_px`).
fn pct_limits_to_px(e: &Element, inherited: &Computed) -> Element {
    let of = |l: Option<Len>, base: Option<Len>| match (l, base) {
        (Some(Len::Pct(k)), Some(Len::Px(b))) if b > 0.0 => Some(Len::Px(k * b)),
        _ => l,
    };
    let block_cb = matches!(inherited.display, None | Some(Display::Block));
    let h_base = block_cb.then_some(inherited.height).flatten();
    let mut copy = e.clone();
    copy.style.max_width = of(e.style.max_width, inherited.width);
    copy.style.min_width = of(e.style.min_width, inherited.width);
    copy.style.max_height = of(e.style.max_height, h_base);
    copy.style.min_height = of(e.style.min_height, h_base);
    copy
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-flexreplaced-2026-09.md`):
// «использованный размер замещаемого достаётся КОРОБКЕ, а не только
// рисунку» — предпочтительное соотношение на коробку элемента,
// упругий рисунок в гибком контейнере, выведенная сторона в коробку.
// Разбор скаута верен и подтверждён тремя снимками (фон коробки
// стоит верно, рисунок остаётся полоской; зелёный квадрат 200×200
// вместо 100×100), спека — css-flexbox-1 §9.2 п. B, §9.8 п.3, §9.4,
// Blink `ComputeReplacedSizeInternal` + `GetReplacedAspectRatio`.
// Широкий срез 3112 пар (гибкие коробки с картинками), база тем же
// списком: 2367 -> 2367. Вариант из четырёх блоков — **+0 / −0**,
// полный из шести — **+1 / −1**: взята
// `image-fractional-height-with-wide-aspect-ratio`, потеряна
// `box-sizing-007` 0.00 -> 2.50. Ожидание скаута было +6…+10.
// Возвращать только с разбором `box-sizing-007`: соотношение на
// коробку складывается с `border-box` в двойной учёт рамок.
/// То же, но с базовым кеглем для разрешения долей: атом строится от СЫРОГО
/// стиля, и `padding-right: 1em` без разрешения терялся вовсе
/// (wm-propagation-body-040: сосед вставал на 16 точек левее эталона).
/// `object-view-box` (css-images-4 §object-view-box, `csswg-drafts/css-images-4/
/// Overview.bs` «object-view-box»): видимая часть природного объекта
/// становится его НОВЫМ природным размером, а рисуется только она — вырез
/// растягивается на коробку (`object-fit: fill`). Вид записи сводится к
/// прямоугольнику в точках природного растра: `inset(t r b l)`, `rect(t r b l)`
/// (правый и нижний края от левого/верхнего края объекта), `xywh(x y w h)`.
/// Коробка: размер из обособления (`contain-intrinsic-size`) или заданный,
/// иначе из выреза — с соотношением выреза для одной заданной стороны.
/// Только растр (`background::Source::Raster`); иначе — прежний путь.
fn view_boxed(e: &Element, vb: (u8, [Len; 4])) -> Option<AnyElement> {
    let src = e.attr("src")?;
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let source = crate::paint::background::source(&crate::paint::background::key(
        local.unwrap_or(src),
        &e.style,
    ))?;
    let nat = source.intrinsic();
    let (w0, h0) = (nat.w?, nat.h?);
    let crate::paint::background::Source::Raster(ready) = source else {
        return None;
    };
    let at = |l: Len, base: f32| match l {
        Len::Px(v) => v,
        Len::Pct(k) => k * base,
        _ => 0.0,
    };
    let [a, b, c, d] = vb.1;
    let (vx, vy, vw, vh) = match vb.0 {
        // inset(top right bottom left)
        0 => (
            at(d, w0),
            at(a, h0),
            w0 - at(d, w0) - at(b, w0),
            h0 - at(a, h0) - at(c, h0),
        ),
        // rect(top right bottom left)
        1 => (
            at(d, w0),
            at(a, h0),
            at(b, w0) - at(d, w0),
            at(c, h0) - at(a, h0),
        ),
        // xywh(x y w h)
        _ => (at(a, w0), at(b, h0), at(c, w0), at(d, h0)),
    };
    if !(vw > 0.0 && vh > 0.0) {
        return None;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (bw, bh) = if e.style.contains_width() || e.style.contains_height() {
        (
            px_of(e.style.width)
                .or(e.style.contain_intrinsic.0)
                .unwrap_or(0.0),
            px_of(e.style.height)
                .or(e.style.contain_intrinsic.1)
                .unwrap_or(0.0),
        )
    } else {
        match (px_of(e.style.width), px_of(e.style.height)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * vh / vw),
            (None, Some(h)) => (h * vw / vh, h),
            (None, None) => (vw, vh),
        }
    };
    let mut boxed = e.clone();
    boxed.style.object_view_box = None;
    boxed.style.width = Some(Len::Px(bw));
    boxed.style.height = Some(Len::Px(bh));
    boxed.style.contain_size = Some(false);
    boxed.style.contain_inline_size = Some(false);
    let (sx, sy) = (bw / vw, bh / vh);
    let picture = gpui::img(ready)
        .preserve_natural_pixels(true)
        .pixelated(e.style.image_pixelated == Some(true))
        .absolute()
        .left(px(-vx * sx))
        .top(px(-vy * sy))
        .w(px(w0 * sx))
        .h(px(h0 * sy))
        .object_fit(gpui::ObjectFit::Fill);
    let window = div()
        .size_full()
        .relative()
        .overflow_hidden()
        .child(picture);
    Some(
        styled_div(&boxed)
            .flex_shrink_0()
            .child(window)
            .into_any_element(),
    )
}
