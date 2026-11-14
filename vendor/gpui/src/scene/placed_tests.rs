//! KaminIDE patch: a placed range equals the placed copy of the same commands.

use super::*;
use crate::scene::replay_tests::{destination, quad, quads, record, rect, summary};

fn at(x: f32, y: f32) -> Point<ScaledPixels> {
    point(ScaledPixels(x), ScaledPixels(y))
}

/// A quad under a mask of its own.
fn masked(x: f32, y: f32, w: f32, h: f32, mask: Bounds<ScaledPixels>) -> Quad {
    Quad { content_mask: ContentMask { bounds: mask }, ..quad(x, y, w, h) }
}

fn orders(scene: &Scene) -> Vec<DrawOrder> {
    scene
        .paint_operations
        .iter()
        .filter_map(|operation| match operation {
            PaintOperation::Primitive(primitive) => Some(primitive.order()),
            _ => None,
        })
        .collect()
}

const PROBES: [(f32, f32, f32, f32); 3] = [(115., 115., 2., 2.), (105., 155., 3., 3.), (300., 100., 5., 5.)];

fn probe(scenes: [&mut Scene; 2]) {
    for scene in scenes {
        for (x, y, w, h) in PROBES {
            scene.insert_primitive(quad(x, y, w, h));
        }
    }
}

#[test]
fn the_whole_range_is_the_placed_record() {
    let clip = Some(rect(90., 90., 150., 150.));
    let record = record();
    let all = 0..record.paint_operations.len();

    let mut whole = destination();
    assert!(whole.replay_placed(&record, at(100., 100.), clip));
    let mut ranged = destination();
    assert!(ranged.replay_placed_range(&record, all, at(100., 100.), PlacedMask::Moves(clip)));

    assert_eq!(summary(&ranged), summary(&whole));
    probe([&mut ranged, &mut whole]);
    assert_eq!(summary(&ranged), summary(&whole));
    ranged.finish();
    whole.finish();
    assert_eq!(quads(&ranged), quads(&whole));
}

/// What `Window::paint_plan` wrote in two copies: the record into an isolated
/// scene under the plan's mask, that scene into the frame.
#[test]
fn a_record_placed_through_a_mask_is_its_isolated_copy_placed() {
    let record = record();
    let extent = rect(0., 0., 45., 70.);
    let clip = rect(90., 90., 150., 150.);
    let mut isolated = Scene::default();
    assert!(isolated.replay_placed(&record, Point::default(), Some(extent)));
    let mut two_step = destination();
    assert!(two_step.replay_placed(&isolated, at(100., 100.), Some(clip)));
    let mut direct = destination();
    assert!(direct.replay_record_placed(&record, extent, at(100., 100.), clip));

    assert_eq!(summary(&direct), summary(&two_step));
    probe([&mut direct, &mut two_step]);
    assert_eq!(summary(&direct), summary(&two_step));
    direct.finish();
    two_step.finish();
    assert_eq!(quads(&direct), quads(&two_step));
}

/// The first two commands of the record, placed as a range, are the placed
/// record of those two commands alone, whether the range has an index of
/// its own or is packed at each use.
#[test]
fn a_range_is_the_placed_record_of_its_commands() {
    let clip = Some(rect(90., 90., 150., 150.));
    let mut alone = Scene::default();
    alone.insert_primitive(quad(0., 0., 40., 40.));
    alone.insert_primitive(quad(10., 10., 40., 40.));
    alone.finish();
    let mut whole = destination();
    assert!(whole.replay_placed(&alone, at(100., 100.), clip));

    let mut source = record();
    let mut packed = destination();
    assert!(packed.replay_placed_range(&source, 0..2, at(100., 100.), PlacedMask::Moves(clip)));
    source.index_range(0..2);
    source.index_range(0..2);
    assert_eq!(source.blocks.len(), 1);
    let mut grafted = destination();
    assert!(grafted.replay_placed_range(&source, 0..2, at(100., 100.), PlacedMask::Moves(clip)));
    assert!(matches!(grafted.blocks.last().map(|b| &b.index), Some(BlockIndex::Owned(_))));

    assert_eq!(summary(&packed), summary(&whole));
    assert_eq!(summary(&grafted), summary(&whole));
    probe([&mut packed, &mut whole]);
    probe([&mut grafted, &mut Scene::default()]);
    assert_eq!(summary(&packed), summary(&whole));
    assert_eq!(summary(&grafted), summary(&whole));
}

/// A range from the middle of a scene keeps the order of its commands and
/// lies above what it covers and below what is painted over it later.
#[test]
fn a_later_range_keeps_the_overlap_rule() {
    let source = record();
    // The layer with its two quads: commands 2..6.
    let mut scene = Scene::default();
    scene.insert_primitive(quad(100., 150., 30., 30.));
    let under = orders(&scene)[0];
    assert!(scene.replay_placed_range(&source, 2..6, at(100., 100.), PlacedMask::Moves(None)));
    scene.insert_primitive(quad(105., 155., 3., 3.));
    scene.insert_primitive(quad(500., 500., 3., 3.));
    let orders = orders(&scene);
    assert_eq!(orders.len(), 5);
    assert!(under < orders[1] && orders[1] < orders[2], "{orders:?}");
    assert!(orders[2] < orders[3], "a quad over the placed layer is above it: {orders:?}");
    assert!(orders[4] < orders[1], "a quad away from it is not raised: {orders:?}");
}

#[test]
fn a_range_that_cannot_move_as_one_is_declined() {
    let mut bottom = Scene::default();
    bottom.push_bottom_layer();
    bottom.insert_primitive(quad(0., 0., 10., 10.));
    bottom.pop_layer();
    bottom.finish();
    let source = record();
    let mut scene = destination();
    let before = scene.paint_operations.len();
    let moves = PlacedMask::Moves(None);
    assert!(!bottom.placeable(0..bottom.paint_operations.len(), &moves));
    assert!(!scene.replay_placed_range(&bottom, 0..bottom.paint_operations.len(), at(0., 0.), moves));
    // Half a layer, and a range past the end.
    assert!(!scene.replay_placed_range(&source, 2..4, at(0., 0.), moves));
    assert!(!scene.replay_placed_range(&source, 0..100, at(0., 0.), moves));
    assert_eq!(scene.paint_operations.len(), before);
}

/// Under an open layer of the destination the range is still placed: its
/// commands are inserted one by one and take the layer's order.
#[test]
fn a_range_under_an_open_layer_is_inserted_by_command() {
    let source = record();
    let mut scene = destination();
    let before = scene.paint_operations.len();
    scene.push_layer(rect(0., 0., 300., 300.));
    let clip = Some(rect(90., 90., 150., 150.));
    assert!(scene.replay_placed_range(&source, 0..2, at(100., 100.), PlacedMask::Moves(clip)));
    assert_eq!(scene.paint_operations.len(), before + 3);
    assert_eq!(scene.blocks.len(), 0);
    let PaintOperation::StartLayer(_, layer) = scene.paint_operations[before] else {
        panic!("a layer");
    };
    assert_eq!(orders(&scene)[before..], [layer, layer]);
    let PaintOperation::Primitive(Primitive::Quad(moved)) = &scene.paint_operations[before + 1] else {
        panic!("a quad");
    };
    assert_eq!(moved.transformation.translation, [100., 100.]);
    assert_eq!(moved.content_mask.bounds, rect(100., 100., 40., 40.));
}

/// Content moved under a mask that stays: the mask is kept, what leaves it
/// is dropped, and a part the mask hid at the old place is indexed at the
/// new one.
#[test]
fn a_fixed_mask_stays_while_its_content_moves() {
    let view = rect(0., 0., 100., 100.);
    let mut frame = Scene::default();
    frame.insert_primitive(masked(0., 0., 50., 20., view));
    frame.insert_primitive(masked(0., 90., 50., 40., view));
    frame.finish_frame();
    let all = 0..frame.paint_operations.len();
    let fixed = PlacedMask::Fixed(view);

    assert!(frame.placeable(all.clone(), &fixed));
    assert!(!frame.placeable(all.clone(), &PlacedMask::Fixed(rect(0., 0., 100., 90.))));
    let mut next = Scene::default();
    assert!(!next.replay_placed_range(&frame, all.clone(), at(0., -30.5), fixed));
    assert!(next.paint_operations.is_empty() && next.blocks.is_empty());

    // Up by 30: the first quad leaves the view, the second is whole in it.
    assert!(next.replay_placed_range(&frame, all, at(0., -30.), fixed));
    assert_eq!(next.paint_operations.len(), 1);
    let PaintOperation::Primitive(Primitive::Quad(moved)) = &next.paint_operations[0] else {
        panic!("a quad");
    };
    assert_eq!(moved.content_mask.bounds, view);
    // An untransformed quad moves its bounds, as a fresh paint writes it.
    assert_eq!(moved.bounds, rect(0., 60., 50., 40.));
    assert_eq!(moved.transformation, TransformationMatrix::unit());
    // y 70..100 of the moved quad was below the view before the move.
    next.insert_primitive(masked(0., 80., 10., 10., view));
    let orders = orders(&next);
    assert!(orders[0] < orders[1], "{orders:?}");
}
