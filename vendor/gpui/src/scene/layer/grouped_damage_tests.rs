//! The layer damage of a replayed frame that draws effect groups
//! (`Scene::note_damage`, `grouped_damage`).

use super::{Binding, LayerHandle, LayerValue, SceneLayer};
use crate::scene::{PaintGroup, SceneDamage};
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, hsla, point, size};
use std::rc::Rc;

const W: u32 = 400;
const H: u32 = 400;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

fn quad(b: Bounds<ScaledPixels>, color: Hsla) -> Quad {
    Quad {
        bounds: b,
        content_mask: ContentMask {
            bounds: rect(0., 0., W as f32, H as f32),
        },
        background: Background::from(color),
        ..Default::default()
    }
}

fn red() -> Hsla {
    hsla(0., 1., 0.5, 1.)
}

fn blue() -> Hsla {
    hsla(0.66, 1., 0.5, 1.)
}

/// A layer whose owner 7 paints one box at (50, 50).
fn layer() -> LayerHandle {
    let mut scene = Scene::default();
    scene.insert_primitive(quad(rect(50., 50., 100., 100.), red()));
    let op = (scene.paint_operations.len() - 1) as u32;
    let bindings = vec![Binding { owner: 7, value: LayerValue::Background, quad: 0, op, opacity: 1. }];
    LayerHandle::new(SceneLayer::finish(scene, bindings, Vec::new()))
}

/// A group at (250, 250) painting one box of `color`.
fn group(color: Hsla) -> PaintGroup {
    let mut inner = Scene::default();
    inner.insert_primitive(quad(rect(250., 250., 50., 50.), color));
    PaintGroup {
        scene: Rc::new(inner),
        bounds: rect(250., 250., 50., 50.),
        ..Default::default()
    }
}

/// The damage of a frame replaying `rendered` after owner 7 turned blue,
/// drawing `next_group` where `rendered` drew a red group.
fn replayed(layer: &LayerHandle, next_group: PaintGroup) -> SceneDamage {
    let mut rendered = Scene::default();
    rendered.insert_layer(layer, rect(0., 0., W as f32, H as f32));
    rendered.groups.push(group(red()));
    rendered.finish();
    rendered.note_frame_damage(&Scene::default(), 1, (W, H));
    rendered.release_layers();
    layer.set_background(7, red(), blue()).expect("owner 7 painted red");
    let mut next = Scene::default();
    next.replay(0..rendered.paint_operations.len(), &rendered);
    next.groups.push(next_group);
    next.finish();
    next.note_damage(&rendered);
    next.note_frame_damage(&rendered, 2, (W, H));
    next.frame_damage().0
}

#[test]
fn a_replayed_frame_whose_groups_paint_the_same_images_is_damaged_where_its_layers_were_written() {
    assert_eq!(
        replayed(&layer(), group(red())),
        SceneDamage::Rect { x: 50, y: 50, width: 100, height: 100 }
    );
}

#[test]
fn a_group_that_paints_another_image_leaves_the_layer_damage_unknown() {
    assert_eq!(replayed(&layer(), group(blue())), SceneDamage::Unknown);
}
