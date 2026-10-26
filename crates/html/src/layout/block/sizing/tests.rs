//! Контракты переноса ограничений без изменения вычисленного стиля.

use super::*;

#[test]
fn transferred_minimum_does_not_override_explicit_maximum() {
    let style = Computed {
        aspect_ratio: Some(1.0),
        min_height: Some(Len::Px(200.0)),
        max_width: Some(Len::Px(100.0)),
        ..Computed::default()
    };
    let resolved = BoxDimensions::from_computed(&style).transfer_ratio(&style);
    assert_eq!(resolved.minimum[0], Some(Len::Px(100.0)));
    assert_eq!(resolved.maximum[1], Some(Len::Px(200.0)));
}

#[test]
fn transferred_maximum_does_not_override_preferred_size() {
    let style = Computed {
        aspect_ratio: Some(2.0),
        width: Some(Len::Px(300.0)),
        min_height: Some(Len::Px(0.0)),
        max_height: Some(Len::Px(100.0)),
        ..Computed::default()
    };
    assert_eq!(
        BoxDimensions::from_computed(&style)
            .transfer_ratio(&style)
            .maximum[0],
        Some(Len::Px(300.0))
    );
}

#[test]
fn transferred_maximum_does_not_cap_content_based_automatic_minimum() {
    let style = Computed {
        aspect_ratio: Some(2.0),
        width: Some(Len::Px(200.0)),
        max_width: Some(Len::Px(100.0)),
        ..Computed::default()
    };
    let input = BoxDimensions::from_computed(&style);
    assert_eq!(input.transfer_ratio(&style), input);
}

#[test]
fn transferred_constraint_preserves_content_box_units() {
    let mut style = Computed {
        aspect_ratio: Some(1.25),
        max_height: Some(Len::Px(80.0)),
        ..Computed::default()
    };
    style.padding.bottom = Some(Len::Px(20.0));
    assert_eq!(
        BoxDimensions::from_computed(&style)
            .transfer_ratio(&style)
            .maximum[0],
        Some(Len::Px(100.0))
    );
}

#[test]
fn percentage_and_auto_inputs_survive_repeated_constraint_transfer() {
    let style = Computed {
        aspect_ratio: Some(2.0),
        width: Some(Len::Pct(0.5)),
        height: Some(Len::Auto),
        min_height: Some(Len::Px(20.0)),
        max_height: Some(Len::Pct(0.75)),
        ..Computed::default()
    };
    let input = BoxDimensions::from_computed(&style);
    for _ in 0..2 {
        let result = input.transfer_ratio(&style);
        assert_eq!(result.preferred, [Some(Len::Pct(0.5)), Some(Len::Auto)]);
        assert_eq!(result.minimum[0], Some(Len::Px(40.0)));
        assert_eq!(result.maximum, [None, Some(Len::Pct(0.75))]);
        assert_eq!(BoxDimensions::from_computed(&style), input);
    }
}
