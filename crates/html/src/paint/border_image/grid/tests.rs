//! Regression checks for source overlap and deferred border-image geometry.

use super::*;
use crate::style::computed::{BorderImageSlice, Computed};
use gpui::{point, px, size};

#[test]
fn overlapping_source_cuts_stay_anchored_to_opposite_edges() {
    let mut c = Computed::default();
    c.apply_one("border-image", "linear-gradient(green, green) 80 / 20px");
    let mut image = c.border_image.unwrap();
    image.slice = [BorderImageSlice::Px(80.0); 4];
    let source = Source::Gradient {
        raw: "linear-gradient(green, green)".into(),
    };
    let area = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
    let grid = Grid::new(&image, &source, area, [10.0; 4]);
    assert_eq!(grid.src_cols, [80.0, 0.0, 80.0]);
    assert_eq!(grid.source_x, [0.0, 80.0, 20.0]);
    assert_eq!(grid.source_y, [0.0, 80.0, 20.0]);
    assert_eq!(grid.cols, [20.0, 60.0, 20.0]);
}

#[test]
fn shorthand_resets_width_and_preserves_outset_multipliers() {
    let declarations = crate::style::css::parse_decls(
        "border-image-width: 3; border-image: url(x.png) 25% / / 2 5px",
    );
    let c = Computed::resolve(&mut vec![], &declarations);
    let image = c.border_image.unwrap();
    assert_eq!(image.width, [BorderImageWidth::Times(1.0); 4]);
    assert_eq!(image.outset[0].px(20.0, 100.0), 40.0);
    assert_eq!(image.outset[1].px(20.0, 100.0), 5.0);
}
