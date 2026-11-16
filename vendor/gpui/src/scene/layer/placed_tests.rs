//! Scene layers: the box runs a paint record carries into the scenes it is
//! placed in. A box in N placements has N runs; each recorded again where it
//! was placed (`place_run`) leaves the layer drawing what a paint from
//! scratch of the record with the box's new commands draws.

use super::order_tests::{drawn, env, quad, rect};
use super::{Binding, LayerHandle, LayerValue, Placed, PlacedMap, SceneLayer, layer_counters, place_run};
use crate::scene::{PaintGroup, draw_sequence};
use crate::{Hsla, Scene, ScaledPixels, hsla, point};
use std::sync::atomic::Ordering::Relaxed;

fn grey() -> Hsla { hsla(0., 0., 0.9, 1.) }
fn blue() -> Hsla { hsla(0.6, 1., 0.5, 1.) }
fn red() -> Hsla { hsla(0., 1., 0.5, 1.) }

/// Two columns of a child 100 tall, 50 per column, 60 apart.
fn columns() -> [Placed; 2] {
    let at = |x: f32, y: f32| point(ScaledPixels(x), ScaledPixels(y));
    [
        Placed { offset: at(0., 0.), clip: Some(rect(0., 0., 40., 50.)) },
        Placed { offset: at(60., -50.), clip: Some(rect(60., 0., 40., 50.)) },
    ]
}

/// A child's paint record: its fill, box 1 in the second column, box 2
/// split over both, a quad after them; box 2's fill `fill` is bound.
fn record(fill: Hsla, layered: bool) -> Scene {
    let mut s = Scene { in_group: true, ..Scene::default() };
    if layered {
        s.book.record_runs = Some(Box::default());
    }
    s.insert_primitive(quad(rect(0., 0., 40., 100.), grey()));
    for (owner, area, color) in [(1, rect(0., 60., 40., 30.), blue()), (2, rect(0., 40., 40., 20.), fill)] {
        let open = s.open_box(owner, area, env());
        assert_eq!(open, layered, "a record that keeps runs holds its boxes");
        if owner == 2 && layered {
            let (op, quad) = (s.paint_operations.len() as u32, s.quads.len() as u32);
            let binding = Binding { owner, value: LayerValue::Background, quad, op, opacity: 1. };
            s.book.record_runs.as_mut().unwrap().bindings.push(binding);
        }
        s.insert_primitive(quad(area, color));
        if open {
            let run = s.close_box().unwrap();
            s.book.record_runs.as_mut().unwrap().boxes.push(run);
        }
    }
    s.insert_primitive(quad(rect(0., 95., 40., 5.), red()));
    s.end_record_runs();
    s.finish();
    s
}

/// The record placed in both columns of a page, with the runs and bindings
/// it carried (`layered`).
fn page(record: &Scene, layered: bool) -> (Scene, super::RecordRuns) {
    let mut s = Scene::default();
    s.insert_primitive(quad(rect(0., 0., 100., 50.), hsla(0., 0., 1., 1.)));
    let mut carried = super::RecordRuns::default();
    for placed in columns() {
        if layered {
            s.book.placed_map = Some(PlacedMap::new(Some(placed), 0));
        }
        assert!(s.replay_placed(record, placed.offset, placed.clip));
        if let (Some(map), Some(runs)) = (s.book.placed_map.take(), record.book.record_runs.as_deref()) {
            let c = runs.carry(&map, &s);
            carried.boxes.extend(c.boxes);
            carried.bindings.extend(c.bindings);
        }
    }
    (s, carried)
}

fn layer(fill: Hsla) -> LayerHandle {
    let (scene, carried) = page(&record(fill, true), true);
    LayerHandle::new(SceneLayer::finish(scene, carried.bindings, carried.boxes))
}

/// Box `owner`'s runs recorded again with `fill` as `Window::rerecord_layer_box`
/// does: painted in the record's coordinates, placed as each run was.
fn rerecord(layer: &LayerHandle, owner: u32, fill: Hsla) -> usize {
    let runs: Vec<usize> = layer.0.content.borrow().runs_of(owner).collect();
    for &at in &runs {
        let b = layer.0.content.borrow().boxes[at].clone();
        let mut again = Scene::default();
        assert!(again.open_box(owner, b.area, b.env));
        again.insert_primitive(quad(rect(0., 40., 40., 20.), fill));
        again.close_box();
        let (again, bindings) = place_run(again, Vec::new(), b.placed.expect("a carried run is placed")).unwrap();
        let plan = layer.0.content.borrow().check_box(at, &again).expect("the run is replaced");
        let mut current = layer.0.content.borrow_mut();
        std::rc::Rc::make_mut(&mut current).replace_box(at, again, bindings, plan);
    }
    runs.len()
}

fn draws(layer: &LayerHandle) -> Vec<(String, crate::Bounds<ScaledPixels>)> {
    let mut out = Vec::new();
    draw_sequence(layer.content().scene(), &mut out);
    out
}

#[test]
fn a_record_placed_in_two_columns_draws_as_without_runs() {
    let (mut plain, _) = page(&record(blue(), false), false);
    assert_eq!(draws(&layer(blue())), drawn(&mut plain));
}

#[test]
fn a_box_in_two_columns_has_a_run_in_each_and_one_where_it_shows() {
    let layer = layer(blue());
    let content = layer.content();
    assert_eq!(content.runs_of(2).count(), 2, "box 2 shows in both columns");
    assert_eq!(content.runs_of(1).count(), 1, "box 1 shows in the second column only");
    let placed: Vec<_> = content.runs_of(2).map(|at| content.boxes[at].placed).collect();
    assert_eq!(placed, columns().map(Some).to_vec());
}

#[test]
fn a_box_recorded_again_in_every_column_draws_as_a_paint_from_scratch() {
    let layer = layer(blue());
    for fill in [red(), grey(), blue()] {
        assert_eq!(rerecord(&layer, 2, fill), 2);
        let (mut plain, _) = page(&record(fill, false), false);
        assert_eq!(draws(&layer), drawn(&mut plain), "box 2 recorded again as {fill:?}");
    }
}

#[test]
fn a_bound_fill_binds_its_quad_in_each_column() {
    let layer = layer(blue());
    let content = layer.content();
    let at = |q: &crate::Quad| crate::scene::placed_bounds(&crate::scene::Primitive::Quad(*q));
    let ours: Vec<_> = content.bindings.iter().filter(|b| b.owner == 2).collect();
    let mut bound: Vec<_> = ours.iter().map(|b| at(&content.scene.quads[b.quad as usize])).collect();
    bound.sort_by(|a, b| a.origin.x.0.total_cmp(&b.origin.x.0));
    // Box 2's fill as placed in each column (moved by the placement's transform).
    let expected = [rect(0., 40., 40., 20.), rect(60., -10., 40., 20.)];
    assert_eq!(bound, expected.to_vec());
    for binding in ours {
        let quad = at(&content.scene.quads[binding.quad as usize]);
        let op = &content.scene.paint_operations[binding.op as usize];
        let same = matches!(op, crate::scene::PaintOperation::Primitive(crate::scene::Primitive::Quad(q)) if at(q) == quad);
        assert!(same, "the binding's command is its quad");
    }
}

#[test]
fn a_group_in_a_record_drops_the_runs_it_kept() {
    let dropped = layer_counters::RUNS_UNPLACED.load(Relaxed);
    let mut s = Scene { in_group: true, ..Scene::default() };
    s.book.record_runs = Some(Box::default());
    assert!(s.open_box(1, rect(0., 0., 10., 10.), env()));
    s.insert_primitive(quad(rect(0., 0., 10., 10.), blue()));
    let run = s.close_box().unwrap();
    s.book.record_runs.as_mut().unwrap().boxes.push(run);
    s.groups.push(PaintGroup { scene: Scene::default(), bounds: rect(0., 0., 1., 1.), ..Default::default() });
    s.end_record_runs();
    let runs = s.book.record_runs.as_deref().expect("the record says whom it does not hold");
    assert!(runs.boxes.is_empty() && runs.bindings.is_empty(), "a record with a group carries no runs");
    assert_eq!(runs.unheld, vec![1], "the owner of the run kept before the group is unheld");
    assert!(layer_counters::RUNS_UNPLACED.load(Relaxed) > dropped, "the dropped run is counted");
}

#[test]
fn a_box_painted_per_fragment_keeps_no_fill_and_is_not_replaceable() {
    let fill = |x: f32| crate::Bounds { origin: point(crate::px(x), crate::px(0.)), size: crate::size(crate::px(12.), crate::px(10.)) };
    let mut s = Scene { in_group: true, ..Scene::default() };
    s.book.record_runs = Some(Box::default());
    assert!(s.open_box(1, rect(0., 0., 30., 10.), env()));
    s.note_open_box_fill(fill(0.));
    s.insert_primitive(quad(rect(0., 0., 12., 10.), blue()));
    let one = s.close_box().unwrap();
    assert!(one.replaceable && one.env.fill == Some(fill(0.)), "one fill is the box's fill");
    assert!(s.open_box(2, rect(0., 0., 30., 10.), env()));
    s.note_open_box_fill(fill(0.));
    s.insert_primitive(quad(rect(0., 0., 12., 10.), blue()));
    s.note_open_box_fill(fill(13.));
    s.insert_primitive(quad(rect(13., 0., 12., 10.), blue()));
    let split = s.close_box().unwrap();
    assert!(!split.replaceable, "a box with a fill per fragment has no one fill to restore");
}

/// Box 2's command placed twice in one scene (op 2 lands as ops 2 and 3).
fn twice(record: &Scene) -> super::RecordRuns {
    let [one, two] = columns();
    let mut map = Some(PlacedMap::new(None, record.paint_operations.len()));
    PlacedMap::set(&mut map, 2, 2, Some(2), one);
    PlacedMap::set(&mut map, 2, 3, Some(3), two);
    let mut dest = Scene::default();
    for _ in 0..4 {
        dest.insert_primitive(quad_at());
    }
    record.book.record_runs.as_deref().unwrap().carry(map.as_ref().unwrap(), &dest)
}

#[test]
fn a_box_placed_twice_in_one_scene_and_once_in_another_is_not_held() {
    let record = record(blue(), true);
    let unheld = twice(&record).unheld;
    assert_eq!(unheld, vec![2, 2], "its run and its binding are unheld");
    let finish = |release: bool| {
        let (scene, mut carried) = page(&record, true);
        if release {
            super::release_unheld(&mut carried.bindings, &mut carried.boxes, unheld.clone());
        }
        LayerHandle::new(SceneLayer::finish(scene, carried.bindings, carried.boxes))
    };
    // Without the release the copies in the other scene would take the write.
    let kept = finish(false);
    assert!(kept.holds_box(2) && kept.set_background(2, blue(), red()).is_some());
    let layer = finish(true);
    assert!(!layer.holds_box(2), "no run of box 2 is recorded again");
    assert!(layer.set_background(2, blue(), red()).is_none(), "the write refuses: the layer paints anew");
    assert!(layer.holds_box(1), "box 1 stays held");
}

#[test]
fn a_record_placed_as_one_copy_unholds_every_owner_it_kept() {
    let record = record(blue(), true);
    let mut owners = record.book.record_runs.as_deref().unwrap().dropped().unheld;
    owners.sort_unstable();
    assert_eq!(owners, vec![1, 2]);
}

#[test]
fn a_run_placed_twice_in_one_scene_is_dropped_and_counted() {
    let record = record(blue(), true);
    let [one, two] = columns();
    // Box 2's command (op 2) landed twice: once by each placement.
    let mut map = Some(PlacedMap::new(None, record.paint_operations.len()));
    PlacedMap::set(&mut map, 2, 2, Some(2), one);
    PlacedMap::set(&mut map, 2, 3, Some(3), two);
    let dropped = layer_counters::RUNS_UNPLACED.load(Relaxed);
    let mut dest = Scene::default();
    for _ in 0..4 {
        dest.insert_primitive(quad_at());
    }
    let carried = record.book.record_runs.as_deref().unwrap().carry(map.as_ref().unwrap(), &dest);
    assert!(carried.boxes.iter().all(|b| b.owner != 2), "a run placed twice is not the layer's");
    assert!(carried.unheld.contains(&2), "its owner is unheld");
    assert!(layer_counters::RUNS_UNPLACED.load(Relaxed) > dropped);
}

fn quad_at() -> crate::Quad { quad(rect(0., 0., 1., 1.), grey()) }
