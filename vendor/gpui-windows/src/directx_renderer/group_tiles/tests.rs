//! KaminIDE patch: tests of the tile lists of `group_tiles`.

use super::bins::spans;
use super::{BlurQuad, CoverShape, Region, TILE_CAP, TileLists, bin};

fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
    Region { x, y, width, height }
}

#[test]
fn every_tile_lists_its_composites_in_document_order() {
    let mut l = TileLists::default();
    // 40 x 20 target: 3 x 2 tiles of 16 px.
    let n = bin(&[r(0, 0, 20, 10), r(10, 5, 30, 15), r(33, 17, 1, 1)], &[], 40, 20, TILE_CAP, &mut l);
    assert_eq!(n, 3);
    let tile = |t: usize| &l.data[l.data[t] as usize..l.data[t + 1] as usize];
    assert_eq!(tile(0), [1, 2]);
    assert_eq!(tile(1), [1, 2]);
    assert_eq!(tile(2), [2]);
    assert_eq!(tile(3), [2]);
    assert_eq!(tile(4), [2]);
    assert_eq!(tile(5), [2, 3]);
    assert_eq!(l.data.len(), 7 + 9);
}

#[test]
fn a_full_tile_ends_the_pass_before_the_composite_that_overflows_it() {
    let mut l = TileLists::default();
    let stack = vec![r(0, 0, 8, 8); 5];
    assert_eq!(bin(&stack, &[], 32, 32, 3, &mut l), 3);
    assert_eq!(&l.data[l.data[0] as usize..l.data[1] as usize], [1, 2, 3]);
    // Disjoint composites never split.
    let apart: Vec<Region> = (0..4).map(|i| r(i * 16, 0, 16, 16)).collect();
    assert_eq!(bin(&apart, &[], 64, 16, 1, &mut l), 4);
}

#[test]
fn a_turned_plane_is_listed_and_drawn_only_where_its_image_reaches() {
    // Frame to raster x' = x - y: the raster square 0..100 lies on the
    // parallelogram (0,0) (100,0) (200,100) (100,100) of a 208 x 112 target.
    let mut q = BlurQuad::default();
    q.proj = [[1.0, -1.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]];
    q.source_rect = [0.0, 0.0, 100.0, 100.0];
    let shape = CoverShape::of(&q, 208.0, 112.0);
    let mut l = TileLists::default();
    assert_eq!(bin(&[r(0, 0, 200, 100)], &[shape], 208, 112, TILE_CAP, &mut l), 1);
    let listed = |x: usize, y: usize| l.data[y * 13 + x + 1] > l.data[y * 13 + x];
    // Row 5 (y 80..96): the image spans x 80..196, the box 0..200.
    assert!(!listed(0, 5) && !listed(3, 5));
    assert!(listed(4, 5) && listed(12, 5));
    // Row 0: x 0..116.
    assert!(listed(0, 0) && listed(7, 0) && !listed(9, 0));
    // The pass draws one span per row, cut to its cover.
    let cover = r(0, 0, 200, 100);
    let drawn = spans(&l.data, 208, cover);
    assert_eq!(drawn.len(), 7);
    assert_eq!(drawn[0], r(0, 0, 128, 16));
    assert_eq!(drawn[5].x, 64);
    assert_eq!(drawn[6], r(80, 96, 120, 4));
    // Without a shape every tile of the box is listed.
    bin(&[r(0, 0, 200, 100)], &[None], 208, 112, TILE_CAP, &mut l);
    assert_eq!(spans(&l.data, 208, cover), (0..7).map(|y| r(0, y * 16, 200, if y == 6 { 4 } else { 16 })).collect::<Vec<_>>());
}