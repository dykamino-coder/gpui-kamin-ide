//! Картинки `<img>` и замещаемые с источником-изображением: сборка GPUI-элемента по
//! стилю (`image_with`), сведение долей высоты и пределов к точкам от содержащего блока
//! до раскладки, `object-view-box` (css-images-4) как новый природный размер; локальные,
//! вписанные, заданного и авто-размера пути — в подмодулях.

use crate::dom::Element;
use crate::paint::box_tree::styled::styled_div;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, div, px};
mod contained_size;
mod used_size;
mod view_box;
pub(crate) use used_size::{Picture, PictureSizing};
use view_box::{ViewBoxSize, view_box_size};
mod with;
pub(crate) use with::image_with;
mod grid_measured;
pub(crate) use grid_measured::inherit_item_metadata;
mod content_radii;
mod filtered;
mod local;
mod piped;
use local::{fit_local_image, local_image_source};
mod contained;
use contained::contained_image;
mod auto_sized;
mod sized;

pub(crate) fn image(e: &Element, cx: &crate::layout::cx::LayoutCx) -> AnyElement {
    image_with(e, None, cx)
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
    let mut resolved = pct_limits_to_px(e, inherited);
    // Replaced boxes resolve viewport units from their raw style, too.
    resolved.style.viewport_unit_insets = resolved
        .style
        .viewport_unit_insets
        .or(inherited.viewport_unit_insets);
    let e = &resolved;
    let (Some(Len::Pct(k)), Some(Len::Px(h))) = (e.style.height, inherited.height) else {
        return e.clone();
    };
    // `inline-block` сюда пускается: картинка СТРОКИ внутри `inline-block` с
    // высотой в точках иначе рисуется природным размером — доля высоты до
    // раскладки не доезжает (`intrinsic-percent-replaced-021`: `<button>`
    // 100px, картинка 200×200 вместо 100×100; тот же `div` блоком — верно).
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
        let px_of = px_lossy_zero;
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

// Не отдавать использованный размер замещаемого КОРОБКЕ, а не только
// рисунку (предпочтительное соотношение на коробку элемента, упругий
// рисунок в гибком контейнере, выведенная сторона в коробку;
// css-flexbox-1 §9.2 п. B, §9.8 п.3, §9.4, Blink
// `ComputeReplacedSizeInternal` + `GetReplacedAspectRatio`) без разбора
// `box-sizing-007`: соотношение на коробку складывается с `border-box` в
// двойной учёт рамок (выигрыш — `image-fractional-height-with-wide-
// aspect-ratio`).
/// То же, но с базовым кеглем для разрешения долей: атом строится от СЫРОГО
/// стиля, и `padding-right: 1em` без разрешения терялся вовсе
/// (wm-propagation-body-040: сосед вставал на 16 точек левее эталона).
/// `object-view-box` (css-images-4 §object-view-box, `csswg-drafts/css-images-4/
/// Overview.bs` «object-view-box»): видимая часть природного объекта
/// становится его НОВЫМ природным размером, а рисуется только она — вырез
/// растягивается на коробку (`object-fit: fill`). Вырез и коробка —
/// `view_box_size`. Только растр (`background::Source::Raster`); иначе —
/// прежний путь.
fn view_boxed(
    e: &Element,
    vb: (u8, [Len; 4]),
    cx: &crate::layout::cx::LayoutCx,
) -> Option<AnyElement> {
    let src = e.attr("src")?;
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let source = crate::paint::background::element_source(local.unwrap_or(src), &e.style)?;
    let nat = source.intrinsic();
    let (w0, h0) = (nat.w?, nat.h?);
    let crate::paint::background::Source::Raster(ready) = source else {
        return None;
    };
    let ViewBoxSize {
        view: (vx, vy, vw, vh),
        size: (bw, bh),
    } = view_box_size(&e.style, vb, (w0, h0))?;
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
        styled_div(&boxed, cx)
            .flex_shrink_0()
            .child(window)
            .into_any_element(),
    )
}
