//! Фон вычисляет длины без изображения и сохраняет процентную часть при наследовании.

use crate::style::cascade::preparation::computed;
use crate::style::computed::{BgSize, Computed};
use crate::style::values::value::{Len, calc_get};

#[test]
fn scalar_background_lengths_compute_without_an_image_and_inherit_absolutely() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    for image in ["none", "url(a)", "url(a), url(b)"] {
        let mut own = Computed::default();
        own.apply_one("font-size", "20px", vcx);
        own.apply_one("background-image", image, vcx);
        own.apply_one("background-position", "calc(50% + 2em) 3em", vcx);
        own.apply_one("background-size", "calc(25% + 4em) 5em", vcx);
        let parent = computed(&Computed::default(), &own, vcx);
        let Some(Len::Calc(position)) = parent.bg_pos.x else {
            panic!("percentage must remain in computed position: {image}")
        };
        assert_eq!(
            calc_get(position, vcx).length_percentage(),
            Some((0.5, 40.0))
        );
        assert_eq!(parent.bg_pos.y, Some(Len::Px(60.0)));
        let BgSize::Fixed(Some(Len::Calc(size)), Some(Len::Px(height))) = parent.bg_size else {
            panic!("percentage must remain in computed size: {image}")
        };
        assert_eq!(calc_get(size, vcx).length_percentage(), Some((0.25, 80.0)));
        assert_eq!(height, 100.0);
        let mut own = Computed::default();
        own.apply_one("font-size", "80px", vcx);
        own.apply_one("background-position", "inherit", vcx);
        own.apply_one("background-size", "inherit", vcx);
        let child = computed(&parent, &own, vcx);
        assert_eq!(child.bg_pos, parent.bg_pos);
        assert_eq!(child.bg_size, parent.bg_size);
    }
}

#[test]
fn scalar_background_line_units_compute_without_an_image() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let mut own = Computed::default();
    own.apply_one("font-size", "20px", vcx);
    own.apply_one("line-height", "2", vcx);
    own.apply_one("background-position", "1lh calc(50% + 2lh)", vcx);
    own.apply_one("background-size", "3lh 50%", vcx);
    let result = computed(&Computed::default(), &own, vcx);
    assert_eq!(result.bg_pos.x, Some(Len::Px(40.0)));
    let Some(Len::Calc(position)) = result.bg_pos.y else {
        panic!("percentage must remain in computed position")
    };
    assert_eq!(
        calc_get(position, vcx).length_percentage(),
        Some((0.5, 80.0))
    );
    assert_eq!(
        result.bg_size,
        BgSize::Fixed(Some(Len::Px(120.0)), Some(Len::Pct(0.5)))
    );
}
