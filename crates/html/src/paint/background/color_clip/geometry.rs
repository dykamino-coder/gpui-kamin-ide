//! Геометрия внутреннего фона: сначала нормализация внешних радиусов,
//! затем вычитание рамки и padding отдельно по каждой оси (§4.3, §4.5).

use crate::document::state::ValueCx;
use crate::style::computed::{BgClip, Computed};
use gpui::{Background, Bounds, PathBuilder, Pixels, Window, point, px};

type Radii = [(f32, f32); 4];

pub(super) fn area(
    c: &Computed,
    area: super::ColorBox,
    clip: BgClip,
    vcx: ValueCx<'_>,
) -> (Bounds<Pixels>, Radii) {
    let (padding, border, pad) = match area {
        super::ColorBox::Padding {
            bounds,
            border,
            padding,
        } => (bounds, border, padding),
        // CSS 2.1 §17.5.1 / CSS Backgrounds §4.6: внутренние полосы
        // таблицы не имеют padding и скруглённых краёв.
        super::ColorBox::TableBand(bounds) => return (bounds, [(0.0, 0.0); 4]),
    };
    let border = [border.top, border.right, border.bottom, border.left].map(f32::from);
    let pad = [pad.top, pad.right, pad.bottom, pad.left].map(f32::from);
    let [t, r, b, l] = border;
    let outer = Bounds {
        origin: gpui::point(padding.left() - px(l), padding.top() - px(t)),
        size: gpui::size(
            padding.size.width + px(l + r),
            padding.size.height + px(t + b),
        ),
    };
    let spec = super::super::rrect_spec(c, None, vcx);
    let radii = super::super::shape_raster::parse_rrect(
        spec.trim_start_matches("rrect(").trim_end_matches(')'),
        outer.size.width.into(),
        outer.size.height.into(),
        1.0,
    )
    .map(|r| r.corners)
    .unwrap_or([(0.0, 0.0); 4]);
    let cuts = match clip {
        BgClip::BorderBox => [0.0; 4],
        // В схлопнутой модели border представлен половиной кромки внутри
        // layout padding. Для content-box она уже входит в полный отступ.
        BgClip::ContentBox => std::array::from_fn(|i| border[i] + pad[i]),
        _ => c.bg_border_insets.unwrap_or(border),
    };
    inset(outer, radii, cuts)
}

pub(super) fn inset(
    outer: Bounds<Pixels>,
    radii: Radii,
    [t, r, b, l]: [f32; 4],
) -> (Bounds<Pixels>, Radii) {
    let bounds = Bounds {
        origin: gpui::point(outer.left() + px(l), outer.top() + px(t)),
        size: gpui::size(
            (outer.size.width - px(l + r)).max(px(0.0)),
            (outer.size.height - px(t + b)).max(px(0.0)),
        ),
    };
    let cuts = [(l, t), (r, t), (r, b), (l, b)];
    (
        bounds,
        std::array::from_fn(|i| {
            (
                (radii[i].0 - cuts[i].0).max(0.0),
                (radii[i].1 - cuts[i].1).max(0.0),
            )
        }),
    )
}

pub(super) fn paint_contour(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radii: Radii,
    fill: Background,
) {
    use super::super::contour::{Seg, Shaped, outer};
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let contour = outer(&Shaped {
        rect: (bounds.left().into(), bounds.top().into(), w, h),
        radii,
        k: [1.0; 4],
    });
    let p = |(x, y)| point(px(x), px(y));
    let mut path = PathBuilder::fill();
    path.move_to(p(contour.start));
    for segment in contour.segs {
        match segment {
            Seg::Line(to) => path.line_to(p(to)),
            Seg::Cubic(a, b, to) => path.cubic_bezier_to(p(to), p(a), p(b)),
        }
    }
    path.close();
    // paint_path принимает уже преобразованную геометрию, в отличие от
    // paint_quad; матрица окна задана в физических пикселях устройства.
    let matrix = window.current_transformation();
    let [[a, c], [b, d]] = matrix.rotation_scale;
    let [x, y] = matrix.translation;
    let scale = window.scale_factor();
    path.transform(gpui::Transform::new(a, b, c, d, x / scale, y / scale));
    if let Ok(path) = path.build() {
        window.paint_path(path, fill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::background::ColorBox;

    #[test]
    fn internal_table_band_has_no_padding_border_or_corner_radius() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("border", "20px solid blue", vcx);
        c.apply_one("padding", "10%", vcx);
        c.apply_one("border-radius", "50%", vcx);
        let bounds = Bounds {
            origin: point(px(25.0), px(30.0)),
            size: gpui::size(px(140.0), px(90.0)),
        };
        for clip in [BgClip::BorderBox, BgClip::PaddingBox, BgClip::ContentBox] {
            assert_eq!(
                area(&c, ColorBox::TableBand(bounds), clip, vcx),
                (bounds, [(0.0, 0.0); 4])
            );
        }
    }

    #[test]
    fn inner_corners_keep_each_outer_center_and_allow_empty_content() {
        let outer = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: gpui::size(px(100.0), px(80.0)),
        };
        let (bounds, radii) = inset(
            outer,
            [(50.0, 40.0), (40.0, 30.0), (30.0, 20.0), (20.0, 10.0)],
            [10.0, 20.0, 30.0, 40.0],
        );
        assert_eq!(bounds.size, gpui::size(px(40.0), px(40.0)));
        assert_eq!(radii, [(10.0, 30.0), (20.0, 20.0), (10.0, 0.0), (0.0, 0.0)]);
        assert_eq!(
            inset(outer, [(0.0, 0.0); 4], [60.0; 4]).0.size,
            gpui::size(px(0.0), px(0.0))
        );
    }

    #[test]
    fn content_clip_uses_layout_padding_instead_of_resolving_percentage_again() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("padding", "10%", vcx);
        // Containing block = 400px, content = 100px: used padding = 40px.
        let area_box = ColorBox::Padding {
            bounds: Bounds {
                origin: point(px(3.0), px(3.0)),
                size: gpui::size(px(180.0), px(130.0)),
            },
            border: gpui::Edges::all(px(3.0)),
            padding: gpui::Edges::all(px(40.0)),
        };
        let (content, _) = area(&c, area_box, BgClip::ContentBox, vcx);
        assert_eq!(content.origin, point(px(43.0), px(43.0)));
        assert_eq!(content.size, gpui::size(px(100.0), px(50.0)));
    }

    #[test]
    fn collapsed_half_border_is_already_inside_the_layout_padding() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.bg_border_insets = Some([10.0; 4]);
        c.apply_one("padding", "15px", vcx);
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: gpui::size(px(100.0), px(80.0)),
        };
        let padding = area(
            &c,
            ColorBox::Padding {
                bounds,
                border: gpui::Edges::all(px(0.0)),
                padding: gpui::Edges::all(px(15.0)),
            },
            BgClip::PaddingBox,
            vcx,
        )
        .0;
        let content = area(
            &c,
            ColorBox::Padding {
                bounds,
                border: gpui::Edges::all(px(0.0)),
                padding: gpui::Edges::all(px(15.0)),
            },
            BgClip::ContentBox,
            vcx,
        )
        .0;
        assert_eq!(padding.origin, point(px(10.0), px(10.0)));
        assert_eq!(padding.size, gpui::size(px(80.0), px(60.0)));
        assert_eq!(content.origin, point(px(15.0), px(15.0)));
        assert_eq!(content.size, gpui::size(px(70.0), px(50.0)));
    }
}
