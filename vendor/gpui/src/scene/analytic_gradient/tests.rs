//! Replay compares and owns stop content independently of scene-local GPU indices.
use super::*;
use crate::{
    Background, BackgroundTag, ContentMask, ScaledPixels, SceneDamage, bounds, point, size,
};

fn gradient(color: f32) -> Arc<AnalyticGradient> {
    AnalyticGradient::new(
        [1., 0., 16., 0.],
        [16., 16.],
        false,
        false,
        [[color, 0., 0., 1., 0.], [0., 0., 1., 1., 1.]],
    )
    .unwrap()
}
fn paint(scene: &mut Scene, g: Arc<AnalyticGradient>) {
    let b = bounds(
        point(ScaledPixels(0.), ScaledPixels(0.)),
        size(ScaledPixels(16.), ScaledPixels(16.)),
    );
    let mut background = Background::default();
    background.tag = BackgroundTag::AnalyticGradient;
    background.solid.a = 1.;
    scene.insert_primitive(Primitive::Gradient(GradientQuad {
        quad: Quad {
            bounds: b,
            content_mask: ContentMask { bounds: b },
            background,
            ..Default::default()
        },
        gradient: g,
    }));
}
#[test]
fn equal_content_shares_descriptor_and_finished_copy_keeps_payload() {
    let mut s = Scene::default();
    paint(&mut s, gradient(1.));
    paint(&mut s, gradient(1.));
    s.finish();
    assert_eq!(s.gradient_data().descriptors.len(), 1);
    assert_eq!(s.gradient_data().stops.len(), 2);
    let copy = s.copy_finished();
    assert_eq!(copy.gradient_data().stops[0].color[0], 1.);
    let mut a = Vec::new();
    let mut b = Vec::new();
    super::super::draw_sequence(&s, &mut a);
    super::super::draw_sequence(&copy, &mut b);
    assert_eq!(a, b);
}
#[test]
fn replay_and_group_batch_remap_indices_without_changing_content() {
    let mut a = Scene::default();
    paint(&mut a, gradient(1.));
    a.finish();
    let mut b = Scene::default();
    paint(&mut b, gradient(0.));
    b.replay(0..a.paint_operations.len(), &a);
    b.finish();
    assert_eq!(b.gradient_data().descriptors.len(), 2);
    let mut data = GradientData::default();
    let mut qa = a.quads.clone();
    data.append(&a, &mut qa);
    let mut qb = b.quads.clone();
    data.append(&b, &mut qb);
    assert_eq!(qa[0].background.stop_count, 0);
    for q in &qb {
        let d = data.descriptors[q.background.stop_count as usize];
        let original = b.gradient(q.background.stop_count - 1).unwrap();
        assert_eq!(
            data.stops[d.first as usize].color[0].to_bits(),
            original.stops[0][0]
        );
    }
}
#[test]
fn changed_stops_damage_even_with_same_local_index_and_equal_stops_ignore_index() {
    let mut a = Scene::default();
    paint(&mut a, gradient(1.));
    a.finish();
    a.note_frame_damage(&Scene::default(), 1, (32, 32));
    let mut b = Scene::default();
    paint(&mut b, gradient(0.));
    b.finish();
    b.note_frame_damage(&a, 2, (32, 32));
    assert_ne!(b.frame_damage().0, SceneDamage::Unchanged);
    let mut c = Scene::default();
    paint(&mut c, gradient(0.));
    paint(&mut c, gradient(1.));
    assert!(a.same_gradient_quad(&a.quads[0], &c, &c.quads[1]));
}
#[test]
fn removing_last_command_releases_payload_and_compacts_live_indices() {
    let mut s = Scene::default();
    let dead = gradient(1.);
    let weak = Arc::downgrade(&dead);
    paint(&mut s, dead);
    paint(&mut s, gradient(0.));
    s.paint_operations.remove(0);
    s.quads.remove(0);
    s.compact_gradients();
    assert!(weak.upgrade().is_none());
    assert_eq!(s.quads[0].background.stop_count, 0);
    assert_eq!(s.gradient_data().descriptors.len(), 1);
    assert_eq!(s.gradient_data().stops[0].color[0], 0.);
}
#[test]
fn malformed_geometry_and_unordered_stops_are_refused() {
    assert!(AnalyticGradient::new([0.; 4], [0., 1.], false, false, [[0.; 5]]).is_none());
    assert!(
        AnalyticGradient::new(
            [0.; 4],
            [1.; 2],
            false,
            false,
            [[0., 0., 0., 1., 1.], [0., 0., 0., 1., 0.]]
        )
        .is_none()
    );
}
