//! Тесты разбора border-image в Computed.

use super::*;

/// Сокращение несёт источник, срез и укладку разом.
#[test]
fn shorthand_carries_source_slice_and_repeat() {
    let mut c = Computed::default();
    c.apply_one("border-image", "url(C:/tmp/border.png) 27 round");
    let bi = c.border_image.expect("рамка-картинка разобрана");
    assert_eq!(bi.src, "C:/tmp/border.png");
    assert_eq!(bi.slice[0], BorderImageSlice::Px(27.0));
    assert_eq!(bi.repeat, (Tiling::Round, Tiling::Round));
}

/// Отдельные свойства дополняют ту же запись.
#[test]
fn longhands_add_up() {
    let mut c = Computed::default();
    c.apply_one("border-image-source", "url(C:/tmp/b.png)");
    c.apply_one("border-image-slice", "30% fill");
    c.apply_one("border-image-repeat", "round space");
    let bi = c.border_image.expect("рамка-картинка разобрана");
    assert!(bi.fill);
    assert_eq!(bi.slice[1], BorderImageSlice::Pct(0.3));
    assert_eq!(bi.repeat, (Tiling::Round, Tiling::Space));
}
