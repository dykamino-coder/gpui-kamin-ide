//! Tests of line culling against the content mask.

use super::*;

#[test]
fn only_a_line_whose_grown_band_misses_the_mask_is_skipped() {
    let cull = Cull {
        top: px(100.),
        bottom: px(700.),
        bleed: px(30.),
        far: px(0.),
        left: px(-50.),
        right: px(850.),
    };
    assert!(
        cull.ink_outside((px(20.), px(69.))).is_some(),
        "above, beyond the bleed"
    );
    assert!(
        cull.ink_outside((px(50.), px(71.))).is_none(),
        "its bleed reaches the mask"
    );
    assert!(
        cull.ink_outside((px(400.), px(420.))).is_none(),
        "on screen"
    );
    assert!(
        cull.ink_outside((px(731.), px(750.))).is_some(),
        "below, beyond the bleed"
    );
    assert!(
        cull.ink_outside((px(729.), px(750.))).is_none(),
        "its bleed reaches the mask"
    );
    let ink = cull.ink_outside((px(800.), px(820.))).expect("below");
    assert_eq!(ink.origin, point(px(-50.), px(770.)));
    assert_eq!(ink.size, size(px(900.), px(80.)));
    let farther = Cull {
        far: px(100.),
        ..cull
    };
    assert!(
        farther.ink_outside((px(800.), px(820.))).is_none(),
        "within the far distance"
    );
    let ink = farther
        .ink_outside((px(900.), px(920.)))
        .expect("beyond it");
    assert_eq!(
        ink.origin,
        point(px(-50.), px(870.)),
        "offered grown by the bleed only"
    );
}
