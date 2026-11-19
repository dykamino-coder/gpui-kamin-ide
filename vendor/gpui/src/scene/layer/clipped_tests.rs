//! Scene layers: a box of a placed record whose commands every placement's
//! clip cut away keeps one empty run, as a box painted directly whose
//! commands the content mask culled; a box painted in another placement
//! keeps only its painted runs.

use super::order_tests::{env, quad, rect};
use super::{LayerHandle, Placed, PlacedMap, RecordRuns, SceneLayer};
use crate::{Hsla, Scene, ScaledPixels, hsla, point};

fn blue() -> Hsla {
    hsla(0.6, 1., 0.5, 1.)
}

/// Two columns of a child 100 tall, 50 per column, 60 apart.
fn columns() -> [Placed; 2] {
    let at = |x: f32, y: f32| point(ScaledPixels(x), ScaledPixels(y));
    [
        Placed { offset: at(0., 0.), clip: Some(rect(0., 0., 40., 50.)) },
        Placed { offset: at(60., -50.), clip: Some(rect(60., 0., 40., 50.)) },
    ]
}

/// A record of box 1 (second column) and box 3 below both columns.
fn record() -> Scene {
    let mut s = Scene { in_group: true, ..Scene::default() };
    s.book.record_runs = Some(Box::default());
    for (owner, area) in [(1, rect(0., 60., 40., 30.)), (3, rect(0., 110., 40., 20.))] {
        assert!(s.open_box(owner, area, env()));
        s.insert_primitive(quad(area, blue()));
        let run = s.close_box().unwrap();
        s.book.record_runs.as_mut().unwrap().boxes.push(run);
    }
    s.end_record_runs();
    s.finish();
    s
}

fn layer() -> LayerHandle {
    let record = record();
    let mut page = Scene::default();
    let mut carried = RecordRuns::default();
    for placed in columns() {
        page.book.placed_map = Some(PlacedMap::new(Some(placed), 0));
        assert!(page.replay_placed(&record, placed.offset, placed.clip));
        let map = page.book.placed_map.take().unwrap();
        carried.extend(record.book.record_runs.as_deref().unwrap().carry(&map, &page));
    }
    LayerHandle::new(SceneLayer::finish(page, carried.bindings, carried.boxes))
}

#[test]
fn a_box_clipped_away_in_every_placement_keeps_one_empty_run() {
    let layer = layer();
    let content = layer.0.content.borrow();
    let runs: Vec<usize> = content.runs_of(3).collect();
    assert_eq!(runs.len(), 1, "one empty run for both placements");
    assert!(content.boxes[runs[0]].ops.is_empty(), "no command of box 3 landed");
    drop(content);
    assert!(layer.holds_box(3), "the layer holds the box clipped away");
    let area = layer.box_area(3).expect("its area");
    assert!(
        !area.intersects(&rect(0., 0., 100., 50.)),
        "its area lies outside the columns: {area:?}"
    );
    assert!(
        layer.set_background(3, blue(), hsla(0., 1., 0.5, 1.)).is_none(),
        "no fill of it is painted: a value write refuses"
    );
}

#[test]
fn a_box_painted_in_one_placement_keeps_only_its_painted_run() {
    let layer = layer();
    let content = layer.0.content.borrow();
    let runs: Vec<usize> = content.runs_of(1).collect();
    assert_eq!(runs.len(), 1, "box 1 shows in the second column only");
    assert!(!content.boxes[runs[0]].ops.is_empty(), "its run holds its fill");
}
