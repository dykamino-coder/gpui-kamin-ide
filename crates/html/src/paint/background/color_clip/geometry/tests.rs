//! Геометрия внутреннего фона и использованные отступы.

use super::*;
use crate::paint::background::ColorBox;

#[test]
fn outer_overlap_factor_precedes_subtracting_border_and_padding() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one("border-radius", "80px / 60px", vcx);
    let box_area = ColorBox::Padding {
        bounds: Bounds::new(point(px(10.0), px(20.0)), gpui::size(px(80.0), px(60.0))),
        border: gpui::Edges::all(px(10.0)),
        padding: gpui::Edges {
            top: px(7.0),
            right: px(31.0),
            bottom: px(13.0),
            left: px(3.0),
        },
    };
    // Outer box = 100×80: §5.5 scales all radii by 100/160 = 0.625.
    let (outer, radii) = area(&c, box_area, BgClip::BorderBox, vcx);
    assert_eq!(outer.size, gpui::size(px(100.0), px(80.0)));
    assert_eq!(radii, [(50.0, 37.5); 4]);
    let (content, radii) = area(&c, box_area, BgClip::ContentBox, vcx);
    assert_eq!(content.origin, point(px(13.0), px(27.0)));
    assert_eq!(content.size, gpui::size(px(46.0), px(40.0)));
    assert_eq!(
        radii,
        [(37.0, 20.5), (9.0, 20.5), (9.0, 14.5), (37.0, 14.5)]
    );
}

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
