//! KaminIDE patch: in-place comparison of frames that record retained layers
//! again (`items::unchanged_layers`) and its early stop.

use super::items::unchanged_layers;
use crate::scene::layer::{LayerHandle, SceneLayer};
use crate::scene::{Scene, SceneDamage};
use crate::{Background, ContentMask, Hsla, Quad, ScaledPixels, blue, bounds, point, red, size};

const W: u32 = 256;
const H: u32 = 256;

fn quad(x: f32, y: f32, w: f32, h: f32, color: Hsla) -> Quad {
    let b = bounds(
        point(ScaledPixels(x), ScaledPixels(y)),
        size(ScaledPixels(w), ScaledPixels(h)),
    );
    Quad {
        bounds: b,
        content_mask: ContentMask {
            bounds: bounds(
                point(ScaledPixels(0.0), ScaledPixels(0.0)),
                size(ScaledPixels(W as f32), ScaledPixels(H as f32)),
            ),
        },
        background: Background::from(color),
        ..Default::default()
    }
}

/// A document layer of `n` boxes, the `odd` box of it blue.
fn document(n: usize, odd: Option<usize>) -> LayerHandle {
    let mut content = Scene::default();
    for k in 0..n {
        let color = if Some(k) == odd { blue() } else { red() };
        content.insert_primitive(quad(10.0, 20.0 * k as f32, 100.0, 10.0, color));
    }
    LayerHandle::new(SceneLayer::finish(content, Vec::new(), Vec::new()))
}

/// A frame: a background, the layer recorded anew, a box over it.
fn frame(layer: &LayerHandle, top: Hsla) -> Scene {
    let mut s = Scene::default();
    s.insert_primitive(quad(0.0, 0.0, W as f32, H as f32, Hsla::white()));
    s.insert_layer(layer, bounds(point(ScaledPixels(0.0), ScaledPixels(0.0)), size(ScaledPixels(W as f32), ScaledPixels(H as f32))));
    s.insert_primitive(quad(200.0, 200.0, 20.0, 20.0, top));
    s.finish();
    s
}

fn in_place(old: &mut Scene, new: &mut Scene) -> SceneDamage {
    old.note_frame_damage(&Scene::default(), 1, (W, H));
    new.note_frame_damage_moved(old, 2, (W, H), Some(Ok((0, 0))));
    new.frame_damage().0
}

#[test]
fn a_layer_recorded_again_unchanged_is_equal_without_reading_its_primitives() {
    let layer = document(12, None);
    let (mut old, mut new) = (frame(&layer, red()), frame(&layer, red()));
    assert_eq!(unchanged_layers(&new, &old), [true]);
    assert_eq!(in_place(&mut old, &mut new), SceneDamage::Unchanged);
}

#[test]
fn an_own_change_beside_an_unchanged_layer_damages_only_itself() {
    let layer = document(12, None);
    let (mut old, mut new) = (frame(&layer, red()), frame(&layer, blue()));
    match in_place(&mut old, &mut new) {
        SceneDamage::Rect { width, height, .. } => {
            assert!(width <= 64 && height <= 64, "{width}x{height}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_changed_layer_is_compared_primitive_by_primitive() {
    let (a, b) = (document(12, None), document(12, Some(3)));
    let (mut old, mut new) = (frame(&a, red()), frame(&b, red()));
    assert_eq!(unchanged_layers(&new, &old), [false]);
    match in_place(&mut old, &mut new) {
        SceneDamage::Rect { y, height, .. } => assert!(y <= 60 && y + height >= 70 && height <= 64),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_redraw_of_about_the_whole_window_stops_the_comparison() {
    let layer = document(12, None);
    let mut old = frame(&layer, red());
    let mut new = frame(&layer, red());
    // The background under everything changes: every cell differs.
    new.quads[0].background = Background::from(blue());
    old.note_frame_damage(&Scene::default(), 1, (W, H));
    assert_eq!(new.plan_shift(&old, (W, H), (0, 0)), Err("redraw_whole"));
}