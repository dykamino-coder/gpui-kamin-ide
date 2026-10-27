//! Общая заливка фона по использованным краям коробки (css-backgrounds-3 §3.7, §5.3).
//! Установка слоя не зависит от display: блоки, атомы, таблицы и replaced
//! используют общий слой; табличные полосы вызывают тот же painter.

mod bounded;
mod fill;
mod geometry;
mod gradient;

use crate::document::state::ValueCx;
use crate::hooks::IntoHooked;
use crate::style::computed::{BgClip, Computed};
use gpui::{AnyElement, Bounds, Corners, Edges, Pixels, Window, px};

/// Использованная область задаётся источником геометрии: обычный слой
/// получает padding-box, табличная полоса — область без padding и радиусов.
#[derive(Clone, Copy)]
pub(crate) enum ColorBox {
    Padding {
        bounds: Bounds<Pixels>,
        border: Edges<Pixels>,
        padding: Edges<Pixels>,
    },
    TableBand(Bounds<Pixels>),
}

/// Filter Effects 2 §2.1, шаг 5: собственный фон рисуется после фильтрации
/// подложки. Квад GPUI предшествует детям, поэтому здесь нужен слой фона,
/// даже когда background-clip оставляет всю border-box.
pub(crate) fn needs_color_layer(c: &crate::style::computed::BoxStyle, vcx: ValueCx<'_>) -> bool {
    match c.color_clip(vcx) {
        Some(BgClip::PaddingBox | BgClip::ContentBox) => true,
        None | Some(BgClip::BorderBox) => c.backdrop_filter_set,
        _ => false,
    }
}

/// Абсолютный слой измеряется от padding-box; painter восстанавливает
/// border-box по использованным краям родителя, независимо от clip.
pub(crate) fn color_layer(
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<AnyElement> {
    if !c.background_paint.paints()
        || !needs_color_layer(c, vcx)
        || crate::paint::background::single_corner::eligible(c)
        || crate::paint::effects::corner_clip::insets(c, vcx).is_some()
        || (c.paint_background().color.is_none()
            && (c.paint_background().gradient.is_none() || c.gradient_as_tile()))
    {
        return None;
    }
    let style = c.clone();
    let values = vcx.keep();
    Some(
        super::exact_layer::ExactLayer::inset([0.0; 4], move |geometry, window| {
            if let Some((border, padding)) = geometry.parent_edges {
                let area = ColorBox::Padding {
                    bounds: geometry.positioning(geometry.snapped),
                    border,
                    padding,
                };
                paint_color_values(
                    &style,
                    &style.paint_background(),
                    area,
                    window,
                    values.vcx(),
                );
            }
        })
        .hooked(),
    )
}

/// Цвет — под изображениями, clip берётся у нижнего слоя. Прямоугольник
/// задан в padding-box; маска ячеек таблицы остаётся ответственностью caller.
pub(crate) fn paint_color(c: &Computed, area: ColorBox, window: &mut Window, vcx: ValueCx<'_>) {
    paint_color_values(
        c,
        &crate::style::background_paint::BackgroundValues::from_computed(c),
        area,
        window,
        vcx,
    );
}

fn paint_color_values(
    c: &Computed,
    bg: &crate::style::background_paint::BackgroundValues,
    area: ColorBox,
    window: &mut Window,
    vcx: ValueCx<'_>,
) {
    let clip = bg.color_clip.unwrap_or(BgClip::BorderBox);
    if matches!(clip, BgClip::Text | BgClip::BorderArea) {
        return;
    }
    let Some(fill) = fill::Fill::new(c, bg, area, vcx) else {
        return;
    };
    let (bounds, radii) = geometry::area(c, area, clip, vcx);
    if bounds.size.width <= px(0.0) || bounds.size.height <= px(0.0) {
        return;
    }
    let bounds = super::sampling::snapped_clip(bounds, window);
    // У GPUI один круглый радиус на угол; эллипсы и большие неодинаковые
    // углы идут общим контуром, чтобы не зажиматься половиной стороны.
    let circular = radii.iter().all(|(x, y)| (x - y).abs() < 1e-5);
    let limit = f32::from(bounds.size.width.min(bounds.size.height)) * 0.5;
    if circular && radii.iter().all(|(x, _)| *x <= limit) {
        let [tl, tr, br, bl] = radii.map(|(x, _)| px(x));
        window.paint_quad(
            gpui::fill(bounds, fill.at(bounds, gradient::IDENTITY)).corner_radii(Corners {
                top_left: tl,
                top_right: tr,
                bottom_right: br,
                bottom_left: bl,
            }),
        );
    } else {
        geometry::paint_contour(window, bounds, radii, &fill);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::style::computed::Display;
    use crate::style::values::value::Len;

    #[test]
    fn backdrop_background_is_painted_after_sampling_for_every_position_and_clip() {
        use gpui::Styled;
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for position in ["static", "relative", "absolute", "fixed", "sticky"] {
            for clip in ["border-box", "padding-box", "content-box"] {
                for filter in ["invert(1)", "blur(2px)", "url(#filter)"] {
                    let mut c = crate::style::computed::BoxStyle::default();
                    c.apply_one("position", position, vcx);
                    c.apply_one("background", &format!("#fff8 {clip}"), vcx);
                    c.apply_one("backdrop-filter", filter, vcx);
                    let mut d = crate::paint::apply::apply_paint(gpui::div(), &c, vcx);
                    assert!(d.style().background.is_none(), "{position} {clip} {filter}");
                    assert!(color_layer(&c, vcx).is_some(), "{clip} {filter}");
                }
            }
        }
        let mut ordinary = crate::style::computed::BoxStyle::default();
        ordinary.apply_one("background", "#fff8", vcx);
        ordinary.apply_one("backdrop-filter", "none", vcx);
        let mut d = crate::paint::apply::apply_paint(gpui::div(), &ordinary, vcx);
        assert!(d.style().background.is_some());
        assert!(color_layer(&ordinary, vcx).is_none());
    }

    #[test]
    fn inset_color_is_installed_for_every_box_kind_even_when_edges_coincide() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        for display in [
            Display::Block,
            Display::InlineBlock,
            Display::Table,
            Display::InlineTable,
            Display::TableCell,
            Display::Flex,
            Display::Grid,
            Display::InlineFlex,
            Display::InlineGrid,
        ] {
            for clip in ["padding-box", "content-box"] {
                let mut c = crate::style::computed::BoxStyle::default();
                c.display = Some(display);
                c.apply_one("background", &format!("blue {clip}"), vcx);
                assert!(color_layer(&c, vcx).is_some(), "{display:?}: {clip}");
                c.context.parent_flex_grid = true;
                assert!(color_layer(&c, vcx).is_some());
            }
        }
    }

    #[test]
    fn content_edge_subtracts_both_border_and_padding_from_each_radius_axis() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = crate::style::computed::BoxStyle::default();
        c.apply_one("border", "10px solid transparent", vcx);
        c.apply_one("padding", "5px 20px", vcx);
        c.apply_one("border-radius", "40px / 30px", vcx);
        let padding = Bounds {
            origin: gpui::point(px(10.0), px(10.0)),
            size: gpui::size(px(180.0), px(100.0)),
        };
        let (bounds, radii) = geometry::area(
            &c,
            ColorBox::Padding {
                bounds: padding,
                border: Edges::all(px(10.0)),
                padding: Edges {
                    top: px(5.0),
                    bottom: px(5.0),
                    left: px(20.0),
                    right: px(20.0),
                },
            },
            BgClip::ContentBox,
            vcx,
        );
        assert_eq!(bounds.origin, gpui::point(px(30.0), px(15.0)));
        assert_eq!(bounds.size, gpui::size(px(140.0), px(90.0)));
        assert_eq!(radii, [(10.0, 15.0); 4]);
    }

    #[test]
    fn bottom_layer_clip_controls_color_independently_of_top_image() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = crate::style::computed::BoxStyle::default();
        c.apply_one("background", "url(a) border-box, blue content-box", vcx);
        assert!(color_layer(&c, vcx).is_some());
        c.apply_one("background", "url(a) content-box, blue border-box", vcx);
        assert!(color_layer(&c, vcx).is_none());
        c.padding.left = Some(Len::Px(10.0));
        c.apply_one("background-clip", "text", vcx);
        assert!(color_layer(&c, vcx).is_none());
    }
}
