//! Картинки `<img>` и замещаемые с источником-изображением: сборка GPUI-элемента по
//! стилю (`image_with`) из растра или вектора источника в размерах, которые решает
//! `layout::replaced::image`; `object-view-box` (css-images-4) как новый природный
//! размер; локальные, вписанные, обособленные и измеряемые пути — в подмодулях.

use crate::dom::Element;
use crate::layout::replaced::image::view_box::{ViewBoxSize, view_box_size};
use crate::paint::box_tree::styled::styled_div;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, StyledImage, div, px};
mod with;
pub(crate) use with::image_with;
mod content_radii;
mod filtered;
mod grid_measured;
mod holder_ratio;
mod local;
mod piped;
use local::{fit_local_image, local_image_source};
mod contained;
use contained::contained_image;

pub(crate) fn image(e: &Element, cx: &crate::layout::cx::LayoutCx) -> AnyElement {
    image_with(e, None, cx)
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
    let vcx = cx.vcx();
    let src = e.attr("src")?;
    let local = (src.starts_with("file:") || src.starts_with('/')).then_some(src);
    let source = crate::resources::css_images::element_source(local.unwrap_or(src), &e.style.computed(), vcx)
        .map(crate::paint::background::Source::from_cpu)?;
    let nat = source.intrinsic();
    let (w0, h0) = (nat.w?, nat.h?);
    let crate::paint::background::Source::Raster(ready) = source else {
        return None;
    };
    let ViewBoxSize {
        view: (vx, vy, vw, vh),
        size: (bw, bh),
    } = view_box_size(&e.style.computed(), vb, (w0, h0))?;
    let mut boxed = e.clone();
    boxed.style.used_mut().object_view_box = None;
    boxed.style.used_mut().width = Some(Len::Px(bw));
    boxed.style.used_mut().height = Some(Len::Px(bh));
    boxed.style.used_mut().contain_size = Some(false);
    boxed.style.used_mut().contain_inline_size = Some(false);
    let (sx, sy) = (bw / vw, bh / vh);
    let ready = crate::paint::background::image_adapter::upload(&ready)?;
    let picture = gpui::img(ready)
        .preserve_natural_pixels(true)
        .pixelated(e.style.cascaded().image_pixelated == Some(true))
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
