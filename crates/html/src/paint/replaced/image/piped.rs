//! Картинка через конвейер: локальный растр под фильтром или маской.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};

#[allow(clippy::result_large_err)]
pub(super) fn piped_image(
    src: &str,
    e: &Element,
    d: gpui::Div,
    local: Option<&str>,
    wants_pipe: bool,
    vcx: ValueCx<'_>,
) -> Result<AnyElement, gpui::Div> {
    let values = vcx.keep();
    if let (true, Some(Len::Px(w)), Some(Len::Px(h))) = (wants_pipe, e.style.width, e.style.height)
    {
        let pos = e
            .style
            .object_position
            .unwrap_or(crate::style::computed::BgPos {
                x: Some(Len::Pct(0.5)),
                y: Some(Len::Pct(0.5)),
            });
        use crate::style::computed::BgSize;
        let bgc = crate::style::computed::Computed {
            bg_image: Some(e.style.content_image().cloned().unwrap_or_else(|| {
                crate::style::computed::ImageSource::Url(local.unwrap_or(src).to_string())
            })),
            bg_repeat: Some(crate::style::computed::BgRepeat::NoRepeat),
            bg_pos: pos,
            // Стиль трубы собран с нуля, и отказ от EXIF-разворота в него надо
            // положить руками: иначе `image-orientation: none` вместе с
            // `object-fit`/`object-position` уходил бы мимо ключа источника.
            image_orient_none: e.style.image_orient_none,
            image_pixelated: e.style.image_pixelated,
            bg_size: match e.style.object_fit.as_deref() {
                Some("contain") => BgSize::Contain,
                Some("cover") => BgSize::Cover,
                Some("none") => BgSize::Auto,
                Some("scale-down") => {
                    // Меньшее из `none` и `contain`: влезает — своим
                    // размером, нет — вписать.
                    let fits = crate::resources::css_images::element_source(
                        local.unwrap_or(src),
                        &e.style,
                        vcx,
                    )
                    .map(crate::paint::background::Source::from_cpu)
                    .map(|s| s.intrinsic())
                    .is_some_and(|i| {
                        i.w.is_some_and(|iw| iw <= w) && i.h.is_some_and(|ih| ih <= h)
                    });
                    if fits { BgSize::Auto } else { BgSize::Contain }
                }
                _ => BgSize::Fixed(Some(Len::Pct(1.0)), Some(Len::Pct(1.0))),
            },
            ..Default::default()
        };
        // `overflow: visible` на замещаемом (HTML §rendering: UA-правило
        // `img { overflow: clip; overflow-clip-margin: content-box }`,
        // css-overflow-3): ЯВНОЕ `visible` выпускает картинку за content
        // box — `object-fit: none` рисуется своим размером целиком,
        // скругление её тоже не режет (`overflow-img`, `-svg`,
        // `-border-radius`: эталон — та же картинка без обрезки).
        // Умолчание (`None`) — UA-шный `clip`, прежний путь ниже.
        let spills = e.style.overflow_x == Some(crate::style::computed::Overflow::Visible)
            && e.style.overflow_y == Some(crate::style::computed::Overflow::Visible);
        if spills {
            let style = bgc;
            let layer = gpui::canvas(
                |_, _, _| {},
                move |bounds: gpui::Bounds<gpui::Pixels>, _, window, _| {
                    let vcx = values.vcx();
                    // Область ОТСЧЁТА — content box, область КРАСКИ — с
                    // запасом во все стороны: плитка одна (`no-repeat`),
                    // и рисуется она ровно своим размером.
                    let reach = px(4096.0);
                    let paint = gpui::Bounds {
                        origin: gpui::point(bounds.origin.x - reach, bounds.origin.y - reach),
                        size: gpui::size(
                            bounds.size.width + reach * 2.0,
                            bounds.size.height + reach * 2.0,
                        ),
                    };
                    crate::paint::background::paint_tiles(&style, bounds, Some(paint), window, vcx);
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full();
            return Ok(d
                .child(div().w(px(w)).h(px(h)).relative().child(layer))
                .into_any_element());
        }
        if let Some(layer) = crate::paint::background::layer(&bgc, vcx) {
            // Внутренняя коробка = content box: поля и рамка остаются
            // на хосте, слой не должен их накрывать.
            return Ok(d
                .child(
                    div()
                        .w(px(w))
                        .h(px(h))
                        .relative()
                        .overflow_hidden()
                        .child(layer),
                )
                .into_any_element());
        }
    }
    Err(d)
}
