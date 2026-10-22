//! Решения о размере рисунка по стилю и природным размерам источника.

use super::{Picture, PictureSizing};
use crate::layout::replaced::natural::Intrinsic;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn natural(w: f32, h: f32) -> Option<Intrinsic> {
    Some(Intrinsic {
        w: Some(w),
        h: Some(h),
        ratio: Some(w / h),
    })
}

#[test]
fn authored_width_transfers_height_through_natural_ratio() {
    let style = Computed {
        width: Some(Len::Px(200.0)),
        ..Default::default()
    };
    let sizing = PictureSizing::new(&style, natural(100.0, 50.0), None);
    assert_eq!(
        sizing.picture(),
        (
            Picture::Used {
                raster: (200.0, 100.0),
                size: (200.0, 100.0),
            },
            None,
        )
    );
}

#[test]
fn a_limit_on_the_derived_side_narrows_the_box() {
    // CSS 2.1 §10.4: `width: 200px; max-height: 50px` over 100×50 is 100×50.
    let style = Computed {
        width: Some(Len::Px(200.0)),
        max_height: Some(Len::Px(50.0)),
        ..Default::default()
    };
    let sizing = PictureSizing::new(&style, natural(100.0, 50.0), None);
    assert_eq!(
        sizing.picture(),
        (
            Picture::Used {
                raster: (100.0, 50.0),
                size: (100.0, 50.0),
            },
            Some((100.0, 50.0)),
        )
    );
}

#[test]
fn both_sides_authored_fill_a_flex_item() {
    let mut style = Computed {
        width: Some(Len::Px(40.0)),
        height: Some(Len::Px(30.0)),
        ..Default::default()
    };
    let sizing = PictureSizing::new(&style, natural(10.0, 10.0), None);
    assert_eq!(
        sizing.picture().0,
        Picture::Authored {
            raster: (40.0, 30.0),
            size: Some((40.0, 30.0)),
        }
    );
    style.flex_item = true;
    let sizing = PictureSizing::new(&style, natural(10.0, 10.0), None);
    assert_eq!(
        sizing.picture().0,
        Picture::Authored {
            raster: (40.0, 30.0),
            size: None,
        }
    );
}

#[test]
fn auto_sides_with_only_a_ratio_take_the_containing_block_width() {
    // CSS 2.1 §10.3.2: both `auto` and a ratio alone — width from the block.
    let style = Computed::default();
    let ratio_only = Some(Intrinsic {
        w: None,
        h: None,
        ratio: Some(2.0),
    });
    let sizing = PictureSizing::new(&style, ratio_only, Some(200.0));
    assert_eq!(
        sizing.picture().0,
        Picture::Used {
            raster: (200.0, 100.0),
            size: (200.0, 100.0),
        }
    );
    let sizing = PictureSizing::new(&style, None, Some(200.0));
    assert_eq!(sizing.picture().0, Picture::Natural);
}

#[test]
fn border_box_limits_leave_the_content_size() {
    let style = Computed {
        border_box: Some(true),
        max_width: Some(Len::Px(60.0)),
        padding: crate::style::computed::Sides {
            left: Some(Len::Px(10.0)),
            right: Some(Len::Px(10.0)),
            ..Default::default()
        },
        ..Default::default()
    };
    let sizing = PictureSizing::new(&style, natural(80.0, 40.0), None);
    assert_eq!(sizing.sub, (20.0, 0.0));
    assert_eq!(
        sizing.picture().0,
        Picture::Used {
            raster: (40.0, 20.0),
            size: (40.0, 20.0),
        }
    );
}
