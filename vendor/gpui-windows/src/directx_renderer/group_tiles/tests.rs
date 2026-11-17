//! KaminIDE patch: tests of the tile lists of `group_tiles`.

use super::{Region, TILE_CAP, TileLists, bin};

fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
    Region { x, y, width, height }
}

#[test]
fn every_tile_lists_its_composites_in_document_order() {
    let mut l = TileLists::default();
    // 40 x 20 target: 3 x 2 tiles of 16 px.
    let n = bin(&[r(0, 0, 20, 10), r(10, 5, 30, 15), r(33, 17, 1, 1)], 40, 20, TILE_CAP, &mut l);
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
    assert_eq!(bin(&stack, 32, 32, 3, &mut l), 3);
    assert_eq!(&l.data[l.data[0] as usize..l.data[1] as usize], [1, 2, 3]);
    // Disjoint composites never split.
    let apart: Vec<Region> = (0..4).map(|i| r(i * 16, 0, 16, 16)).collect();
    assert_eq!(bin(&apart, 64, 16, 1, &mut l), 4);
}
