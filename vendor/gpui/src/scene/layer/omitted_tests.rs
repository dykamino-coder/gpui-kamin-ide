//! Absence needs complete placement evidence for every copy of an owner.

use super::order_tests::{env, quad, rect};
use super::{LayerHandle, Placed, PlacedMap, RecordRuns, SceneLayer};
use crate::{ScaledPixels, Scene, hsla, point};

fn source() -> Scene {
    let mut scene = Scene {
        in_group: true,
        ..Scene::default()
    };
    scene.book.record_runs = Some(Box::default());
    for (owner, x) in [(1, 0.), (2, 20.)] {
        let area = rect(x, 0., 10., 10.);
        assert!(scene.open_box(owner, area, env()));
        scene.insert_primitive(quad(area, hsla(0., 0., 0.5, 1.)));
        let run = scene.close_box().unwrap();
        scene.book.record_runs.as_mut().unwrap().boxes.push(run);
    }
    scene.finish();
    scene
}

fn place(source: &Scene, scene: &mut Scene, clip_x: f32) -> RecordRuns {
    let at = Placed {
        offset: point(ScaledPixels(0.), ScaledPixels(0.)),
        clip: Some(rect(clip_x, 0., 10., 10.)),
    };
    scene.book.placed_map = Some(PlacedMap::new(Some(at), 0));
    assert!(scene.replay_placed(source, at.offset, at.clip));
    let map = scene.book.placed_map.take().unwrap();
    source.book.record_runs.as_ref().unwrap().carry(&map, scene)
}

#[test]
fn omitted_commands_prove_absence_without_a_rerecord_environment() {
    let mut scene = Scene::default();
    let runs = place(&source(), &mut scene, 100.);
    let layer = LayerHandle::new(SceneLayer::finish_recording(scene, runs));
    for owner in [1, 2] {
        assert!(layer.omits_box(owner));
        assert!(!layer.holds_box(owner));
        assert_eq!(layer.box_area(owner), None);
        assert_eq!(
            layer.set_background(owner, hsla(0., 0., 0.5, 1.), hsla(0., 1., 0.5, 1.)),
            None
        );
    }
    assert!(!layer.omits_box(3));
}

#[test]
fn visible_and_unknown_copies_cancel_absence_before_unheld_runs_are_removed() {
    let source = source();
    for unknown in [false, true] {
        let mut scene = Scene::default();
        let mut runs = place(&source, &mut scene, 100.);
        runs.extend(place(&source, &mut scene, 0.));
        if unknown {
            runs.unheld.push(2);
        }
        let layer = LayerHandle::new(SceneLayer::finish_recording(scene, runs));
        assert!(!layer.omits_box(1));
        assert!(layer.holds_box(1));
        assert_eq!(layer.omits_box(2), !unknown);
    }
}

#[test]
fn missing_multiple_empty_and_nonplain_ranges_never_prove_absence() {
    let source = source();
    let runs = source.book.record_runs.as_ref().unwrap();
    let scene = Scene::default();
    assert_eq!(runs.carry(&PlacedMap::new(None, 2), &scene).omitted, [1, 2]);
    let mut many = Some(PlacedMap::new(None, 2));
    for _ in 0..2 {
        PlacedMap::set(&mut many, 0, 0, Some(0), Placed::default());
    }
    let carried = runs.carry(&many.unwrap(), &scene);
    assert!(!carried.omitted.contains(&1));
    assert!(carried.unheld.contains(&1));
    let missing = runs.carry(&PlacedMap::new(None, 0), &scene);
    assert!(missing.omitted.is_empty());
    assert_eq!(missing.unheld, [1, 2]);
    let mut invalid = runs.as_ref().clone();
    invalid.boxes[0].ops = 0..0;
    invalid.boxes[1].replaceable = false;
    let carried = invalid.carry(&PlacedMap::new(None, 2), &scene);
    assert!(carried.omitted.is_empty());
    assert_eq!(carried.unheld, [1, 2]);
}

#[test]
fn another_placement_preserves_absence_but_an_adopted_copy_cancels_it() {
    let mut scene = Scene::default();
    let runs = place(&source(), &mut scene, 100.);
    let carried = runs.carry(&PlacedMap::new(None, 0), &Scene::default());
    assert_eq!(carried.omitted, [1, 2]);
    let dropped = carried.dropped();
    assert!(dropped.omitted.is_empty());
    assert_eq!(dropped.unheld, [1, 2]);
}

#[test]
fn a_changed_footprint_reestablishes_absence_in_a_fresh_recording() {
    let mut scene = Scene::default();
    scene.insert_primitive(quad(rect(0., 0., 100., 100.), hsla(0., 0., 1., 1.)));
    let runs = place(&source(), &mut scene, 0.);
    let layer = SceneLayer::finish_recording(scene, runs);
    let at = layer.runs_of(1).next().unwrap();
    let mut same = Scene::default();
    same.insert_primitive(quad(rect(0., 0., 10., 10.), hsla(0., 1., 0.5, 1.)));
    assert!(layer.check_box(at, &same).is_some());
    let mut changed = Scene::default();
    changed.insert_primitive(quad(rect(0., 0., 15., 10.), hsla(0., 1., 0.5, 1.)));
    assert!(layer.check_box(at, &changed).is_none());
    let mut without_proof = layer.clone();
    without_proof.omitted.clear();
    assert!(without_proof.check_box(at, &changed).is_some());
}
