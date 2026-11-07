use super::{Region, intersects, levels, pack_layers};

#[test]
fn first_fit_layers_are_disjoint_and_cover_every_group_once() {
    // Diagonal chain: neighbours overlap, every third box is clear again.
    let rects: Vec<Region> = (0..30).map(|i| r(i * 40, (i % 5) * 30, 100, 80)).collect();
    let layers = pack_layers(&rects);
    let mut seen: Vec<usize> = layers.concat();
    seen.sort_unstable();
    assert_eq!(seen, (0..30).collect::<Vec<_>>());
    for layer in &layers {
        assert!(layer.windows(2).all(|w| w[0] < w[1]));
        for (a, &i) in layer.iter().enumerate() {
            assert!(layer[a + 1..].iter().all(|&j| !intersects(rects[i], rects[j])));
        }
    }
    // In document order a new layer would start at every overlap (29 times);
    // first-fit needs only as many layers as the deepest overlap.
    assert_eq!(layers.len(), 3);
}

fn r(x: u32, y: u32, width: u32, height: u32) -> Region {
    Region {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn half_open_rectangles_touching_at_an_edge_are_disjoint() {
    assert!(!intersects(r(0, 0, 10, 10), r(10, 0, 5, 5)));
    assert!(!intersects(r(0, 0, 10, 10), r(0, 10, 5, 5)));
    assert!(intersects(r(0, 0, 10, 10), r(9, 9, 5, 5)));
}

#[test]
fn levels_keep_every_overlapping_pair_in_document_order() {
    // A diagonal chain like effects-480: each box overlaps the next two.
    let rects: Vec<Region> = (0..12).map(|i| r(i * 31, i * 47 % 300, 90, 140)).collect();
    let level = levels(&rects);
    for j in 0..rects.len() {
        for i in 0..j {
            if intersects(rects[i], rects[j]) {
                assert!(level[i] < level[j], "{i} {j} {level:?}");
            }
            if level[i] == level[j] {
                assert!(!intersects(rects[i], rects[j]));
            }
        }
    }
    // Disjoint composites share level 0.
    assert_eq!(levels(&[r(0, 0, 5, 5), r(5, 0, 5, 5), r(0, 5, 5, 5)]), [0, 0, 0]);
    assert_eq!(levels(&[r(0, 0, 5, 5), r(2, 2, 5, 5), r(20, 20, 1, 1), r(4, 4, 2, 2)]), [0, 1, 0, 2]);
}
