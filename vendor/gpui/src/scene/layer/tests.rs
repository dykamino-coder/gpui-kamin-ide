//! Scene layers: value writes, copy on write, batching and replay.

use super::{Binding, LayerHandle, LayerValue, SceneLayer, layer_counters};
use crate::scene::{PaintOperation, PrimitiveBatch};
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, hsla, point, size};
use std::sync::atomic::Ordering::Relaxed;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds { origin: point(ScaledPixels(x), ScaledPixels(y)), size: size(ScaledPixels(w), ScaledPixels(h)) }
}

fn quad(b: Bounds<ScaledPixels>, color: Hsla, opacity: f32) -> Quad {
    Quad {
        bounds: b,
        content_mask: ContentMask { bounds: rect(0., 0., 1000., 1000.) },
        background: Background::from(color).opacity(opacity),
        ..Default::default()
    }
}

fn red() -> Hsla {
    hsla(0., 1., 0.5, 1.)
}

fn blue() -> Hsla {
    hsla(0.66, 1., 0.5, 1.)
}

/// Owner 7 paints two quads (one under opacity 0.5), owner 9 one; painted
/// in reverse overlap order so the sort moves them.
fn layer() -> LayerHandle {
    let mut scene = Scene::default();
    let mut bindings = Vec::new();
    let mut paint = |scene: &mut Scene, q: Quad, owner: u32, opacity: f32| {
        let at = scene.quads.len() as u32;
        scene.insert_primitive(q);
        let op = (scene.paint_operations.len() - 1) as u32;
        bindings.push(Binding { owner, value: LayerValue::Background, quad: at, op, opacity });
    };
    paint(&mut scene, quad(rect(0., 0., 100., 100.), red(), 1.), 7, 1.);
    paint(&mut scene, quad(rect(50., 50., 100., 100.), blue(), 1.), 9, 1.);
    paint(&mut scene, quad(rect(120., 120., 10., 10.), red(), 0.5), 7, 0.5);
    LayerHandle::new(SceneLayer::finish(scene, bindings))
}

#[test]
fn a_background_write_changes_only_its_owners_quads_and_returns_their_damage() {
    let layer = layer();
    let before = layer.content().version();
    let damage = layer.set_background(7, red(), blue()).expect("owner 7 painted red");
    assert_eq!(damage, rect(0., 0., 130., 130.));
    let content = layer.content();
    assert_eq!(content.version(), before + 1);
    let fills: Vec<Background> = content.scene().quads.iter().map(|q| q.background).collect();
    assert!(fills.contains(&Background::from(blue()).opacity(0.5)));
    assert_eq!(fills.iter().filter(|f| **f == Background::from(blue())).count(), 2);
    for op in &content.scene().paint_operations {
        if let PaintOperation::Primitive(crate::Primitive::Quad(q)) = op {
            assert_ne!(q.background, Background::from(red()), "commands follow the quads");
        }
    }
}

#[test]
fn a_write_whose_old_value_does_not_match_or_whose_owner_is_unbound_writes_nothing() {
    let layer = layer();
    assert!(layer.set_background(7, blue(), red()).is_none());
    assert!(layer.set_background(8, red(), blue()).is_none());
    assert!(layer.set_background(7, red(), hsla(0., 0., 0., 0.)).is_none());
    assert_eq!(layer.content().version(), 0);
}

#[test]
fn a_held_layer_is_copied_on_write_and_a_released_one_is_not() {
    let layer = layer();
    let mut frame = Scene::default();
    frame.insert_layer(&layer, rect(0., 0., 200., 200.));
    let copies = layer_counters::COW_CLONES.load(Relaxed);
    layer.set_background(9, blue(), red()).unwrap();
    assert_eq!(layer_counters::COW_CLONES.load(Relaxed), copies + 1, "the frame held it");
    assert_eq!(frame.layer(0).unwrap().version(), 0, "the frame keeps what it drew");
    frame.release_layers();
    layer.set_background(9, red(), blue()).unwrap();
    assert_eq!(layer_counters::COW_CLONES.load(Relaxed), copies + 1, "nothing held it");
    frame.hold_layers();
    assert_eq!(frame.layer(0).unwrap().version(), 2, "a held again frame reads the current content");
}

#[test]
fn a_layer_batches_at_its_order_among_the_frame_primitives() {
    let layer = layer();
    let mut frame = Scene::default();
    frame.insert_primitive(quad(rect(0., 0., 1000., 1000.), red(), 1.));
    frame.insert_layer(&layer, rect(0., 0., 200., 200.));
    frame.insert_primitive(quad(rect(10., 10., 10., 10.), blue(), 1.));
    frame.finish();
    let batches: Vec<String> = frame.batches().map(|b| b.label()).collect();
    assert_eq!(batches, ["quads (1)", "layer 0", "quads (1)"]);
    assert!(matches!(frame.batches().nth(1), Some(PrimitiveBatch::Layer(0))));
}

#[test]
fn replaying_a_range_with_a_layer_copies_no_primitive_and_reads_the_current_content() {
    let layer = layer();
    let mut rendered = Scene::default();
    rendered.insert_layer(&layer, rect(0., 0., 200., 200.));
    rendered.finish();
    rendered.release_layers();
    layer.set_background(9, blue(), red()).unwrap();
    let mut next = Scene::default();
    next.replay(0..rendered.paint_operations.len(), &rendered);
    next.finish();
    // The frame took the layer's command, none of its primitives.
    assert!(next.quads.is_empty() && next.paint_operations.len() == 1);
    assert_eq!(next.layer_count(), 1);
    assert_eq!(next.layer(0).unwrap().version(), 1);
    assert_eq!(next.primitive_count(), 3);
}

#[test]
fn a_dropped_layer_replays_as_nothing() {
    let mut rendered = Scene::default();
    {
        let layer = layer();
        rendered.insert_layer(&layer, rect(0., 0., 200., 200.));
        rendered.release_layers();
    }
    let mut next = Scene::default();
    next.replay(0..rendered.paint_operations.len(), &rendered);
    assert_eq!(next.layer_count(), 0);
    assert!(next.paint_operations.is_empty());
}
