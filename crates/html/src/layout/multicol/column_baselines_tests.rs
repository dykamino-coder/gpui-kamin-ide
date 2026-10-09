//! Flow-order baseline selection must survive fragmentation and separate column offsets.

use super::Baselines;

#[test]
fn repeated_bands_surround_the_body_in_content_order_after_source_translation() {
    let mut column = Baselines::default();
    // The header source starts at 100 and is placed in the band at y=20.
    column.include(Some(105.0), Some(105.0), 100.0, 10.0, 20.0);
    column.include(Some(115.0), Some(125.0), 110.0, 40.0, 30.0);
    // The footer is lifted from source y=200 into its visible band at y=80.
    column.include(Some(207.0), Some(207.0), 200.0, 12.0, 80.0);
    assert_eq!(column.first, Some(25.0));
    assert_eq!(column.last, Some(87.0));
}

#[test]
fn whole_block_visible_overflow_and_fragment_clipping_export_different_baselines() {
    let mut whole = Baselines::default();
    whole.include_unclipped(Some(5.0), Some(75.0), 0.0, 10.0);
    assert_eq!(whole.first, Some(15.0));
    assert_eq!(whole.last, Some(85.0));
    let mut clipped = Baselines::default();
    clipped.include(Some(5.0), Some(75.0), 0.0, 20.0, 10.0);
    assert_eq!(clipped.last, Some(15.0));
}

#[test]
fn content_order_wins_over_geometry_inside_one_column() {
    let mut baselines = Baselines::default();
    baselines.include(Some(20.0), Some(30.0), 0.0, 40.0, 50.0);
    baselines.include(Some(5.0), Some(15.0), 0.0, 20.0, 0.0);
    assert_eq!(baselines.first, Some(70.0));
    assert_eq!(baselines.last, Some(15.0));
}

#[test]
fn a_continuation_does_not_export_a_clipped_first_line() {
    let mut baselines = Baselines::default();
    baselines.include(Some(10.0), Some(75.0), 60.0, 30.0, 5.0);
    assert_eq!(baselines.first, Some(20.0));
    assert_eq!(baselines.last, Some(20.0));
}

#[test]
fn columns_and_spanners_export_highest_first_and_lowest_last() {
    let mut baselines = Baselines {
        first: Some(30.0),
        last: Some(70.0),
    };
    baselines.encompass(Baselines {
        first: Some(15.0),
        last: Some(50.0),
    });
    baselines.encompass(Baselines {
        first: Some(100.0),
        last: Some(115.0),
    });
    assert_eq!(baselines.first, Some(15.0));
    assert_eq!(baselines.last, Some(115.0));
}
