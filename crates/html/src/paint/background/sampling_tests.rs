//! Background tile sampling tests.

use super::*;

#[test]
fn resizing_either_axis_or_transforming_retains_interpolation() {
    let identity = [[1.0, 0.0], [0.0, 1.0]];
    assert_eq!(
        sampling_mode((320.0, 320.0), (320.0, 320.0), identity, true),
        ImageSampling::Nearest
    );
    for target in [
        (400.0, 320.0),
        (320.0, 400.0),
        (160.0, 160.0),
        (320.25, 320.0),
    ] {
        assert_eq!(
            sampling_mode((320.0, 320.0), target, identity, true),
            ImageSampling::Linear
        );
    }
    for matrix in [
        [[2.0, 0.0], [0.0, 2.0]],
        [[0.0, -1.0], [1.0, 0.0]],
        [[1.0, 0.5], [0.0, 1.0]],
    ] {
        assert_eq!(
            sampling_mode((320.0, 320.0), (320.0, 320.0), matrix, true),
            ImageSampling::Linear
        );
    }
    assert_eq!(
        sampling_mode((320.0, 320.0), (320.0, 320.0), identity, false),
        ImageSampling::Linear
    );
}
