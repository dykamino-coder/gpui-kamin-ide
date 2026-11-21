//! Pixel coverage, tight damage history and invalid buffer contents.
use super::*;

fn rect(x: u32, y: u32, width: u32, height: u32) -> SceneDamage {
    SceneDamage::Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn distant_edits_preserve_clean_pixels_and_clip_at_the_target() {
    let mut tiles = DamageTiles::default();
    tiles.clear((256, 193));
    tiles.merge(rect(1, 1, 1, 1), &[], (256, 193));
    tiles.merge(rect(255, 192, u32::MAX, u32::MAX), &[], (256, 193));
    let regions = tiles.regions().unwrap();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions.iter().map(|r| r.width * r.height).sum::<u32>(), 2);
    assert_eq!(
        regions[1],
        Region {
            x: 255,
            y: 192,
            width: 1,
            height: 1
        }
    );
    assert!(!DamageTiles::covers_target(&regions, (256, 193)));
}

#[test]
fn history_unions_overlap_once_and_coalesces_tight_vertical_runs() {
    let mut tiles = DamageTiles::default();
    tiles.clear((256, 192));
    tiles.merge(rect(65, 1, 1, 1), &[], (256, 192));
    tiles.merge(rect(65, 1, 1, 127), &[], (256, 192));
    assert_eq!(
        tiles.regions().unwrap(),
        vec![Region {
            x: 65,
            y: 1,
            width: 1,
            height: 127
        }]
    );
    tiles.merge(SceneDamage::Unknown, &[], (256, 192));
    tiles.merge(SceneDamage::Unchanged, &[], (256, 192));
    assert!(tiles.regions().is_none());
    tiles.clear((256, 192));
    assert!(tiles.regions().unwrap().is_empty());
}

#[test]
fn sparse_regions_override_the_conservative_envelope() {
    let mut tiles = DamageTiles::default();
    tiles.clear((256, 192));
    tiles.merge(
        rect(0, 0, 256, 192),
        &[[1, 1, 2, 3], [250, 180, 4, 5]],
        (256, 192),
    );
    assert_eq!(
        tiles
            .regions()
            .unwrap()
            .iter()
            .map(|r| r.width * r.height)
            .sum::<u32>(),
        26
    );
    tiles.merge(SceneDamage::Unchanged, &[], (257, 192));
    assert!(tiles.regions().is_none());
}

#[test]
fn separated_parts_of_neighboring_rows_do_not_fill_a_vertical_gap() {
    let mut tiles = DamageTiles::default();
    tiles.clear((128, 128));
    tiles.merge(rect(3, 3, 4, 2), &[], (128, 128));
    tiles.merge(rect(3, 66, 4, 2), &[], (128, 128));
    let regions = tiles.regions().unwrap();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions.iter().map(|r| r.width * r.height).sum::<u32>(), 16);
}

#[test]
fn cross_cell_strip_keeps_exact_bounds_and_ignores_empty_or_outside_edits() {
    let mut tiles = DamageTiles::default();
    tiles.clear((129, 65));
    for r in [[63, 2, 66, 1], [0, 0, 0, 65], [129, 0, 1, 1], [0, 65, 1, 1]] {
        tiles.merge(rect(r[0], r[1], r[2], r[3]), &[], (129, 65));
    }
    assert_eq!(
        tiles.regions().unwrap(),
        vec![Region {
            x: 63,
            y: 2,
            width: 66,
            height: 1
        }]
    );
}

#[test]
fn every_small_grid_mask_is_covered_exactly_once() {
    let size = (129, 65);
    for mask in 0u32..64 {
        let mut tiles = DamageTiles::default();
        tiles.clear(size);
        for cell in 0..6 {
            if mask & (1 << cell) != 0 {
                tiles.merge(rect((cell % 3) * 64, (cell / 3) * 64, 64, 64), &[], size);
            }
        }
        let regions = tiles.regions().unwrap();
        assert_eq!(DamageTiles::covers_target(&regions, size), mask == 63);
        for y in 0..size.1 {
            for x in 0..size.0 {
                let covers = regions
                    .iter()
                    .filter(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
                    .count();
                let selected = mask & (1 << (x / 64 + (y / 64) * 3)) != 0;
                assert_eq!(covers, usize::from(selected), "mask={mask} at ({x},{y})");
            }
        }
    }
}
