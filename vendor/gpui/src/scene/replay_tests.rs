//! KaminIDE patch: the one-pass placed replay equals copy + replay.

use super::*;

fn quad(x: f32, y: f32, w: f32, h: f32) -> Quad {
    let bounds = Bounds {
        origin: point(ScaledPixels(x), ScaledPixels(y)),
        size: Size { width: ScaledPixels(w), height: ScaledPixels(h) },
    };
    Quad {
        bounds,
        content_mask: ContentMask { bounds },
        transformation: TransformationMatrix::unit(),
        ..Default::default()
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<ScaledPixels> {
    quad(x, y, w, h).bounds
}

/// A finished record: overlapping quads, a layer, one quad that the
/// output clip removes.
fn record() -> Scene {
    let mut scene = Scene::default();
    scene.insert_primitive(quad(0., 0., 40., 40.));
    scene.insert_primitive(quad(10., 10., 40., 40.));
    scene.push_layer(rect(0., 50., 30., 30.));
    scene.insert_primitive(quad(0., 50., 30., 30.));
    scene.insert_primitive(quad(5., 55., 10., 10.));
    scene.pop_layer();
    scene.insert_primitive(quad(200., 0., 10., 10.));
    scene.insert_primitive(quad(20., 20., 5., 5.));
    scene.finish();
    scene
}

/// The destination already holds content under part of the copy.
fn destination() -> Scene {
    let mut scene = Scene::default();
    scene.insert_primitive(quad(100., 100., 30., 30.));
    scene.insert_primitive(quad(110., 110., 30., 30.));
    scene
}

fn summary(scene: &Scene) -> Vec<String> {
    scene
        .paint_operations
        .iter()
        .map(|operation| match operation {
            PaintOperation::Primitive(Primitive::Quad(q)) => format!(
                "quad {} {:?} {:?} {:?}",
                q.order, q.bounds, q.content_mask.bounds, q.transformation.translation
            ),
            PaintOperation::StartLayer(bounds, order) => format!("layer {order} {bounds:?}"),
            PaintOperation::EndLayer => "end".into(),
            _ => "other".into(),
        })
        .collect()
}

fn quads(scene: &Scene) -> Vec<(DrawOrder, [f32; 2])> {
    scene.quads.iter().map(|q| (q.order, q.transformation.translation)).collect()
}

#[test]
fn placed_replay_matches_copy_then_replay() {
    let offset = point(ScaledPixels(100.), ScaledPixels(100.));
    let clip = Some(rect(90., 90., 150., 150.));
    let record = record();

    let mut two_step = destination();
    two_step.replay_record(&record.copy_record(offset, clip));
    let mut fused = destination();
    assert!(fused.replay_placed(&record, offset, clip));

    assert_eq!(summary(&fused), summary(&two_step));
    // The quad at x=200 is moved to 300, outside the clip, and dropped.
    assert_eq!(fused.paint_operations.len(), destination().paint_operations.len() + 7);
    // Later inserts see the copy exactly as they see the two-step copy.
    for probe in [quad(115., 115., 2., 2.), quad(105., 155., 3., 3.), quad(300., 100., 5., 5.)] {
        fused.insert_primitive(probe);
        two_step.insert_primitive(probe);
    }
    assert_eq!(summary(&fused), summary(&two_step));
    fused.finish();
    two_step.finish();
    assert_eq!(quads(&fused), quads(&two_step));
}

#[test]
fn placed_replay_of_a_fully_clipped_record_writes_only_kept_commands() {
    let record = record();
    let clip = Some(rect(1000., 1000., 10., 10.));
    let mut two_step = destination();
    two_step.replay_record(&record.copy_record(Point::default(), clip));
    let mut fused = destination();
    assert!(fused.replay_placed(&record, Point::default(), clip));
    assert_eq!(summary(&fused), summary(&two_step));
}

#[test]
fn placed_replay_declines_records_with_groups_and_open_layers() {
    let mut grouped = record();
    grouped.groups.push(PaintGroup {
        scene: Scene::default(),
        bounds: rect(0., 0., 1., 1.),
        blur_radius: 0.,
        blend: 0,
        polygon: Vec::new(),
        polygon_inverse: false,
        mask: None,
        mask_bounds: rect(0., 0., 1., 1.),
        mask_once: 0,
        mask_clip: None,
        projection: None,
        fragments: Vec::new(),
    });
    let mut scene = destination();
    let before = scene.paint_operations.len();
    assert!(!scene.replay_placed(&grouped, Point::default(), None));
    scene.push_layer(rect(0., 0., 10., 10.));
    assert!(!scene.replay_placed(&record(), Point::default(), None));
    assert_eq!(scene.paint_operations.len(), before + 1);
}

/// A placed copy of a mostly clipped record grows the destination by the
/// commands it keeps, not by the record's length: a frame keeps its
/// capacity, so a reservation for every recorded command would stay dead.
#[test]
fn placed_replay_reserves_only_what_it_keeps() {
    let mut record = Scene::default();
    for i in 0..1000 {
        record.insert_primitive(quad(0., i as f32 * 20., 10., 10.));
    }
    record.finish();
    let mut frame = Scene::default();
    assert!(frame.replay_placed(&record, Point::default(), Some(rect(0., 0., 100., 50.))));
    assert_eq!(frame.paint_operations.len(), 3);
    assert!(frame.paint_operations.capacity() < 100, "{}", frame.paint_operations.capacity());
    let copy = record.copy_record(Point::default(), Some(rect(0., 0., 100., 50.)));
    assert_eq!(copy.paint_operations.len(), 3);
    assert!(copy.paint_operations.capacity() < 100, "{}", copy.paint_operations.capacity());
    // Unclipped (a composition group's copy, kept by the frame): exact.
    let full = record.copy_record(Point::default(), None);
    assert_eq!(full.paint_operations.len(), 1000);
    assert_eq!(full.paint_operations.capacity(), 1000);
}
