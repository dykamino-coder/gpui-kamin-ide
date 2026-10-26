//! Исключения продолженного float отсчитываются от нового логического начала.
use super::*;

#[test]
fn consumed_prefix_does_not_take_space_before_the_resumed_float() {
    for side in [-1, 1] {
        let mut bands = FloatBands::new(120.);
        bands.add_float(-side, 80., 20., None);
        let expected_x = if side < 0 { 0. } else { 60. };
        assert_eq!(
            bands.resume_float(side, 60., 100., -40., 30.),
            (expected_x, -40.)
        );
        assert_eq!(bands.clearance(Some(side), 30.), 60.);
        assert_eq!(bands.clearance(Some(-side), 30.), 30.);
        let shapes = bands.shapes(30.);
        let own = if side < 0 { &shapes.0 } else { &shapes.1 };
        assert_eq!(
            own[0],
            FloatShape::Band {
                top: -70.,
                h: 100.,
                w: 60.
            }
        );
        assert_eq!(bands.available(60., 10.), (0., 120.));
    }
}

#[test]
fn remaining_exclusions_still_obey_float_source_order() {
    let mut bands = FloatBands::new(100.);
    bands.resume_float(-1, 60., 70., -20., 0.);
    assert_eq!(bands.resume_float(1, 60., 70., -30., 0.), (40., 20.));
    assert_eq!(bands.clearance(Some(0), 0.), 90.);
}

#[test]
fn completed_float_is_not_delayed_by_later_exclusions() {
    let mut bands = FloatBands::new(100.);
    bands.resume_float(-1, 100., 100., -10., 0.);
    assert_eq!(bands.resume_float(1, 60., 40., -80., 0.), (40., -80.));
    assert_eq!(bands.clearance(Some(1), 0.), 0.);
}

#[test]
fn narrower_continuation_cannot_overtake_a_delayed_earlier_float() {
    let mut bands = FloatBands::new(100.);
    bands.add_float(-1, 70., 40., None);
    assert_eq!(bands.resume_float(-1, 60., 100., 0., 0.), (0., 40.));
    assert_eq!(bands.resume_float(1, 30., 100., 0., 0.), (70., 40.));
}

#[test]
fn clear_preserves_negative_positions_when_the_named_side_has_no_float() {
    let mut bands = FloatBands::new(100.);
    assert_eq!(bands.clearance(Some(0), -20.), -20.);
    bands.resume_float(-1, 40., 30., -40., 0.);
    assert_eq!(bands.clearance(Some(-1), -20.), -10.);
    assert_eq!(bands.clearance(Some(1), -20.), -20.);
    assert_eq!(bands.clearance(Some(0), -20.), -10.);
    assert_eq!(bands.bottom(None), 0.);
}
