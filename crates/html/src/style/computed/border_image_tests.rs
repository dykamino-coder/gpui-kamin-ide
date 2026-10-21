//! Тесты разбора `border-image` и его отдельных свойств в `Computed`
//! (css-backgrounds-3 §6): сокращение, сложение longhand, `filter()` источником.

use super::*;

/// Сокращение несёт источник, срез и укладку разом.
#[test]
fn shorthand_carries_source_slice_and_repeat() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one("border-image", "url(C:/tmp/border.png) 27 round", vcx);
    let bi = c.border_image.expect("рамка-картинка разобрана");
    assert_eq!(bi.src, Some(ImageSource::Url("C:/tmp/border.png".into())));
    assert_eq!(bi.slice[0], BorderImageSlice::Px(27.0));
    assert_eq!(bi.repeat, (Tiling::Round, Tiling::Round));
}

/// Отдельные свойства дополняют ту же запись.
#[test]
fn longhands_add_up() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one("border-image-source", "url(C:/tmp/b.png)", vcx);
    c.apply_one("border-image-slice", "30% fill", vcx);
    c.apply_one("border-image-repeat", "round space", vcx);
    let bi = c.border_image.expect("рамка-картинка разобрана");
    assert!(bi.fill);
    assert_eq!(bi.slice[1], BorderImageSlice::Pct(0.3));
    assert_eq!(bi.repeat, (Tiling::Round, Tiling::Space));
}

/// `filter(<image>, …)` is one source word (Filter Effects 1 §13).
#[test]
fn shorthand_keeps_filter_image_source() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = Computed::default();
    c.apply_one(
        "border-image",
        "filter(url(C:/tmp/b.png), drop-shadow(50px 0 0 green)) 75 / 75px / 50px",
        vcx,
    );
    let bi = c.border_image.expect("рамка-картинка разобрана");
    let Some(ImageSource::Filter { image, program }) = bi.src else {
        panic!("typed filter image")
    };
    assert_eq!(*image, ImageSource::Url("C:/tmp/b.png".into()));
    assert_eq!(program.shadow.unwrap().x, 50.0);
    assert_eq!(bi.slice[0], BorderImageSlice::Px(75.0));
}
