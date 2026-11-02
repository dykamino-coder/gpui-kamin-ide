use super::*;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

#[test]
fn placed_ink_moves_with_the_box_and_grows_by_the_margin() {
    let ink = rect(-1.0, 2.0, 10.0, 4.0);
    let at = placed(ink, point(ScaledPixels(100.0), ScaledPixels(50.0)), 2.0);
    assert_eq!(at, rect(97.0, 50.0, 14.0, 8.0));
    let screen = rect(0.0, 0.0, 800.0, 600.0);
    assert!(at.intersects(&screen));
    let below = placed(ink, point(ScaledPixels(0.0), ScaledPixels(599.0)), SNAP);
    assert!(
        below.intersects(&screen),
        "the snap margin keeps edge-touching ink"
    );
    let far = placed(ink, point(ScaledPixels(0.0), ScaledPixels(700.0)), SNAP);
    assert!(!far.intersects(&screen));
}
