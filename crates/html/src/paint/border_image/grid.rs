//! Resolve independent source cuts and a shared nine-piece destination grid.
//! CSS Backgrounds 3 §6.2/6.3/6.6; Blink nine_piece_image_grid.cc keeps right
//! and bottom cuts anchored to their own edges even when opposite cuts overlap.

use crate::paint::background::{Source, default_size};
use crate::style::computed::{BorderImage, BorderImageWidth};
use gpui::{Bounds, Pixels};

pub(super) struct Grid {
    pub image: (f32, f32),
    pub area: Bounds<Pixels>,
    pub cols: [f32; 3],
    pub rows: [f32; 3],
    pub src_cols: [f32; 3],
    pub src_rows: [f32; 3],
    pub source_x: [f32; 3],
    pub source_y: [f32; 3],
}

impl Grid {
    pub(super) fn new(
        image: &BorderImage,
        source: &Source,
        area: Bounds<Pixels>,
        base: [f32; 4],
    ) -> Self {
        let (aw, ah) = (f32::from(area.size.width), f32::from(area.size.height));
        let intrinsic = source.intrinsic();
        let (iw, ih) = default_size(intrinsic, (aw, ah));
        let (iw, ih) = (iw.max(1.0), ih.max(1.0));
        let dimensions = [ih, iw, ih, iw];
        let cut = std::array::from_fn::<_, 4, _>(|i| {
            image.slice[i].px(dimensions[i]).clamp(0.0, dimensions[i])
        });
        // Blink resolves auto against the concrete generated image's slices
        // too, after its default object size has established image coordinates.
        let mut w = std::array::from_fn::<_, 4, _>(|i| match image.width[i] {
            BorderImageWidth::Auto => cut[i],
            other => other.px(base[i], [ah, aw, ah, aw][i]),
        });
        let f = (aw / (w[1] + w[3]).max(0.0001))
            .min(ah / (w[0] + w[2]).max(0.0001))
            .min(1.0);
        for value in &mut w {
            *value *= f;
        }
        Self {
            image: (iw, ih),
            area,
            cols: [w[3], (aw - w[3] - w[1]).max(0.0), w[1]],
            rows: [w[0], (ah - w[0] - w[2]).max(0.0), w[2]],
            src_cols: [cut[3], (iw - cut[3] - cut[1]).max(0.0), cut[1]],
            src_rows: [cut[0], (ih - cut[0] - cut[2]).max(0.0), cut[2]],
            source_x: [0.0, cut[3], iw - cut[1]],
            source_y: [0.0, cut[0], ih - cut[2]],
        }
    }
}

#[cfg(test)]
mod tests {
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
}
