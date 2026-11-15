//! Scene layers: value writes, copy on write, batching and replay.

use super::{Binding, LayerHandle, LayerValue, SceneLayer, layer_counters};
use crate::scene::{PaintOperation, PrimitiveBatch};
use crate::{Background, Bounds, ContentMask, Hsla, Quad, ScaledPixels, Scene, hsla, point, size};
use std::sync::atomic::Ordering::Relaxed;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: size(ScaledPixels(w), ScaledPixels(h)),
    }
}

fn quad(b: Bounds<ScaledPixels>, color: Hsla, opacity: f32) -> Quad {
    Quad {
        bounds: b,
        content_mask: ContentMask {
            bounds: rect(0., 0., 1000., 1000.),
        },
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
        bindings.push(Binding {
            owner,
            value: LayerValue::Background,
            quad: at,
            op,
            opacity,
        });
    };
    paint(&mut scene, quad(rect(0., 0., 100., 100.), red(), 1.), 7, 1.);
    paint(
        &mut scene,
        quad(rect(50., 50., 100., 100.), blue(), 1.),
        9,
        1.,
    );
    paint(
        &mut scene,
        quad(rect(120., 120., 10., 10.), red(), 0.5),
        7,
        0.5,
    );
    LayerHandle::new(SceneLayer::finish(scene, bindings, Vec::new()))
}

#[test]
fn a_background_write_changes_only_its_owners_quads_and_returns_their_damage() {
    let layer = layer();
    let before = layer.content().version();
    let damage = layer
        .set_background(7, red(), blue())
        .expect("owner 7 painted red");
    assert_eq!(damage, rect(0., 0., 130., 130.));
    let content = layer.content();
    assert_eq!(content.version(), before + 1);
    let fills: Vec<Background> = content.scene().quads.iter().map(|q| q.background).collect();
    assert!(fills.contains(&Background::from(blue()).opacity(0.5)));
    assert_eq!(
        fills
            .iter()
            .filter(|f| **f == Background::from(blue()))
            .count(),
        2
    );
    for op in &content.scene().paint_operations {
        if let PaintOperation::Primitive(crate::Primitive::Quad(q)) = op {
            assert_ne!(
                q.background,
                Background::from(red()),
                "commands follow the quads"
            );
        }
    }
}

#[test]
fn a_write_whose_old_value_does_not_match_or_whose_owner_is_unbound_writes_nothing() {
    let layer = layer();
    assert!(layer.set_background(7, blue(), red()).is_none());
    assert!(layer.set_background(8, red(), blue()).is_none());
    // An owner with quads has a fill: a write from no fill does not apply.
    assert!(layer.set_backgrounds(&[(7, None, Some(blue()))]).is_none());
    assert_eq!(layer.content().version(), 0);
}

#[test]
fn a_held_layer_is_copied_on_write_and_a_released_one_is_not() {
    let layer = layer();
    let mut frame = Scene::default();
    frame.insert_layer(&layer, rect(0., 0., 200., 200.));
    let copies = layer_counters::COW_CLONES.load(Relaxed);
    layer.set_background(9, blue(), red()).unwrap();
    assert_eq!(
        layer_counters::COW_CLONES.load(Relaxed),
        copies + 1,
        "the frame held it"
    );
    assert_eq!(
        frame.layer(0).unwrap().version(),
        0,
        "the frame keeps what it drew"
    );
    frame.release_layers();
    layer.set_background(9, red(), blue()).unwrap();
    assert_eq!(
        layer_counters::COW_CLONES.load(Relaxed),
        copies + 1,
        "nothing held it"
    );
    frame.hold_layers();
    assert_eq!(
        frame.layer(0).unwrap().version(),
        2,
        "a held again frame reads the current content"
    );
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
    assert!(matches!(
        frame.batches().nth(1),
        Some(PrimitiveBatch::Layer(0))
    ));
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

/// The paint state of a test box.
fn env() -> super::BoxEnv {
    super::BoxEnv {
        bounds: Bounds::default(),
        mask: ContentMask {
            bounds: Bounds::default(),
        },
        transformation: crate::TransformationMatrix::unit(),
        opacity: 1.,
        rem_size: crate::px(16.),
        fill: None,
    }
}

/// Paint box `owner` of `area` as the quads `fills` (the first bound as its
/// background, the others as borders would be) into `scene`.
fn paint_box(
    scene: &mut Scene,
    bindings: &mut Vec<Binding>,
    owner: u32,
    area: Bounds<ScaledPixels>,
    fills: &[(Bounds<ScaledPixels>, Hsla)],
) -> super::BoxRange {
    assert!(scene.open_box(owner, area, env()));
    for (n, &(b, color)) in fills.iter().enumerate() {
        let at = scene.quads.len() as u32;
        scene.insert_primitive(quad(b, color, 1.));
        let op = (scene.paint_operations.len() - 1) as u32;
        if n > 0 {
            continue;
        }
        bindings.push(Binding {
            owner,
            value: LayerValue::Background,
            quad: at,
            op,
            opacity: 1.,
        });
    }
    scene.close_box().expect("the box was open")
}

/// Boxes 1, 2 and 3 side by side with 2, 1 and 2 quads, and a quad of no box over all of them.
fn boxes_layer() -> LayerHandle {
    let (mut scene, mut bindings, mut boxes) = (Scene::default(), Vec::new(), Vec::new());
    let one = rect(0., 0., 100., 20.);
    boxes.push(paint_box(
        &mut scene,
        &mut bindings,
        1,
        one,
        &[(one, red()), (rect(0., 0., 100., 2.), blue())],
    ));
    let two = rect(0., 20., 100., 20.);
    boxes.push(paint_box(
        &mut scene,
        &mut bindings,
        2,
        two,
        &[(two, red())],
    ));
    let three = rect(0., 40., 100., 20.);
    boxes.push(paint_box(
        &mut scene,
        &mut bindings,
        3,
        three,
        &[(three, blue()), (rect(0., 40., 100., 2.), red())],
    ));
    scene.insert_primitive(quad(rect(0., 0., 100., 60.), hsla(0., 0., 0., 0.5), 1.));
    LayerHandle::new(SceneLayer::finish(scene, bindings, boxes))
}

/// Replace box `at`'s run with `fills`, as `Window::rerecord_layer_box` does.
fn rerecord(
    layer: &LayerHandle,
    at: usize,
    fills: &[(Bounds<ScaledPixels>, Hsla)],
) -> Option<bool> {
    let (owner, area) = {
        let content = layer.content();
        let b = &content.boxes[at];
        (b.owner, b.area)
    };
    let (mut scene, mut bindings) = (Scene::default(), Vec::new());
    paint_box(&mut scene, &mut bindings, owner, area, fills);
    let mut current = layer.0.content.borrow_mut();
    let plan = current.check_box(at, &scene)?;
    let (_, moved) = std::rc::Rc::make_mut(&mut current).replace_box(at, scene, bindings, plan);
    Some(moved)
}

/// Each box's quads, read through its commands, and the bound fills.
fn painted(layer: &LayerHandle) -> Vec<(u32, Vec<Bounds<ScaledPixels>>)> {
    let content = layer.content();
    let scene = content.scene();
    // The sorted list holds exactly the commands' quads, in order.
    let mut listed: Vec<String> = scene.quads.iter().map(|q| format!("{q:?}")).collect();
    let mut commanded: Vec<String> = scene
        .paint_operations
        .iter()
        .filter_map(|op| match op {
            PaintOperation::Primitive(crate::Primitive::Quad(q)) => Some(format!("{q:?}")),
            _ => None,
        })
        .collect();
    assert!(
        scene.quads.windows(2).all(|w| w[0].order <= w[1].order),
        "the list stays sorted"
    );
    listed.sort();
    commanded.sort();
    assert_eq!(
        listed, commanded,
        "the sorted list and the commands hold the same quads"
    );
    for b in &content.bindings {
        let q = scene.quads[b.quad as usize];
        let PaintOperation::Primitive(crate::Primitive::Quad(c)) =
            scene.paint_operations[b.op as usize]
        else {
            panic!("a binding names a quad command");
        };
        assert_eq!(
            format!("{q:?}"),
            format!("{c:?}"),
            "a binding's quad is its command's"
        );
    }
    content
        .boxes
        .iter()
        .map(|b| {
            let quads = scene.paint_operations[b.ops.start as usize..b.ops.end as usize]
                .iter()
                .filter_map(|op| match op {
                    PaintOperation::Primitive(crate::Primitive::Quad(q)) => Some(q.bounds),
                    _ => None,
                })
                .collect();
            (b.owner, quads)
        })
        .collect()
}

#[test]
fn boxes_keep_their_runs_apart() {
    let layer = boxes_layer();
    assert_eq!(painted(&layer)[1], (2, vec![rect(0., 20., 100., 20.)]));
}

#[test]
fn a_box_run_is_replaced_one_to_many_many_to_none_and_none_to_one() {
    let layer = boxes_layer();
    let before = painted(&layer);
    let two = rect(0., 20., 100., 20.);
    let fills = [
        (two, blue()),
        (rect(0., 20., 100., 2.), red()),
        (rect(0., 38., 100., 2.), red()),
    ];
    assert_eq!(
        rerecord(&layer, 1, &fills),
        Some(true),
        "later commands moved"
    );
    let after = painted(&layer);
    assert_eq!(after[0], before[0], "the box before kept its quads");
    assert_eq!(after[2], before[2], "the box after kept its quads");
    assert_eq!(after[1].1, fills.iter().map(|f| f.0).collect::<Vec<_>>());
    assert_eq!(layer.content().scene().quads.len(), 8);
    // Its background binds the new quads: a value write reaches them.
    assert!(layer.set_background(2, blue(), red()).is_some());
    assert!(rerecord(&layer, 1, &[]).is_some());
    let none = painted(&layer);
    assert!(none[1].1.is_empty());
    assert_eq!(
        (none[0].clone(), none[2].clone()),
        (before[0].clone(), before[2].clone())
    );
    assert_eq!(layer.content().scene().quads.len(), 5);
    assert!(
        layer.set_background(2, red(), blue()).is_none(),
        "no quad of box 2 is bound"
    );
    assert!(rerecord(&layer, 1, &[(two, red())]).is_some());
    assert_eq!(painted(&layer), before);
    assert!(
        layer.set_background(1, red(), blue()).is_some(),
        "box 1's binding still holds"
    );
}

#[test]
fn a_run_whose_new_ink_raises_a_later_box_moves_it_above() {
    let layer = boxes_layer();
    let before = painted(&layer);
    let moves = layer_counters::RUNS_MOVED_LATER.load(Relaxed);
    // Over box 3's fill, of the new fill's bucket: a paint from scratch
    // raises box 3 above it.
    assert!(rerecord(&layer, 1, &[(rect(0., 20., 100., 30.), red())]).is_some());
    assert!(layer_counters::RUNS_MOVED_LATER.load(Relaxed) > moves);
    let after = painted(&layer);
    assert_eq!((after[0].clone(), after[2].clone()), (before[0].clone(), before[2].clone()));
    let content = layer.content();
    let order = |b: Bounds<ScaledPixels>| content.scene().quads.iter().find(|q| q.bounds == b).map(|q| q.order);
    assert!(order(rect(0., 40., 100., 20.)) > order(rect(0., 20., 100., 30.)));
    // The value binding of box 3 follows its moved quad.
    drop(content);
    assert!(layer.set_background(3, blue(), red()).is_some());
}

/// The run of box 2 (`boxes_layer`) replaced by one quad painted with a
/// border of width `width` and no fill, as a border that appears.
fn rerecord_border(layer: &LayerHandle, width: f32) -> Option<bool> {
    let (owner, area) = {
        let content = layer.content();
        let b = &content.boxes[1];
        (b.owner, b.area)
    };
    let mut scene = Scene::default();
    assert!(scene.open_box(owner, area, env()));
    if width > 0. {
        let mut bordered = quad(rect(0., 20., 100., 20.), hsla(0., 0., 0., 0.), 1.);
        bordered.border_color = blue();
        bordered.border_widths = crate::Edges::all(ScaledPixels(width));
        scene.insert_primitive(bordered);
    }
    scene.close_box().expect("the box was open");
    let mut current = layer.0.content.borrow_mut();
    let plan = current.check_box(1, &scene)?;
    let (_, moved) = std::rc::Rc::make_mut(&mut current).replace_box(1, scene, Vec::new(), plan);
    Some(moved)
}

#[test]
fn a_border_that_appears_and_goes_replaces_only_its_boxs_run() {
    let layer = boxes_layer();
    let before = painted(&layer);
    // 1 → 0: the fill goes; 0 → 1: a border appears; 1 → 0: it goes.
    assert!(rerecord_border(&layer, 0.).is_some());
    assert!(painted(&layer)[1].1.is_empty());
    assert!(rerecord_border(&layer, 2.).is_some());
    let bordered = painted(&layer);
    assert_eq!(bordered[1].1, vec![rect(0., 20., 100., 20.)]);
    let content = layer.content();
    let border = content
        .scene()
        .quads
        .iter()
        .find(|q| q.border_widths.top == ScaledPixels(2.))
        .expect("the border quad is in the sorted list");
    assert_eq!(border.border_color, blue());
    drop(content);
    assert_eq!((bordered[0].clone(), bordered[2].clone()), (before[0].clone(), before[2].clone()));
    assert!(rerecord_border(&layer, 0.).is_some());
    let gone = painted(&layer);
    assert!(gone[1].1.is_empty());
    assert_eq!((gone[0].clone(), gone[2].clone()), (before[0].clone(), before[2].clone()));
    assert!(layer.content().scene().quads.iter().all(|q| q.border_widths.top == ScaledPixels(0.)));
}

#[test]
fn a_group_scene_holds_no_box_run() {
    // A group paints into a scene of its own, composited into the layer as
    // one result: a run there would name commands of another scene.
    let mut scene = Scene::default();
    scene.in_group = true;
    assert!(!scene.open_box(7, rect(0., 0., 10., 10.), env()));
    scene.in_group = false;
    assert!(scene.open_box(7, rect(0., 0., 10., 10.), env()));
}

#[test]
fn a_layer_whose_bindings_name_another_scene_takes_no_writes() {
    // A binding to quad 1 of a scene with one quad (recorded in another
    // scene). Release builds refuse the layer's values instead of indexing.
    if cfg!(debug_assertions) {
        return;
    }
    let mut scene = Scene::default();
    scene.insert_primitive(quad(rect(0., 0., 10., 10.), red(), 1.));
    let bindings = vec![Binding { owner: 7, value: LayerValue::Background, quad: 1, op: 0, opacity: 1. }];
    let before = layer_counters::RECORDS_REFUSED.load(Relaxed);
    let layer = LayerHandle::new(SceneLayer::finish(scene, bindings, Vec::new()));
    assert_eq!(layer_counters::RECORDS_REFUSED.load(Relaxed), before + 1);
    assert_eq!(layer.set_background(7, red(), blue()), None);
}
