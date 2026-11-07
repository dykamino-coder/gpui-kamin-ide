use super::{MAX_DIRECT_SIGMA, direct_radius, margin, weights};

#[test]
fn small_sigmas_convolve_directly_with_three_sigma_support() {
    assert_eq!(direct_radius(0.6), Some(2));
    assert_eq!(direct_radius(1.0), Some(3));
    assert_eq!(direct_radius(MAX_DIRECT_SIGMA), Some(12));
    assert_eq!(direct_radius(4.01), None);
    assert_eq!(direct_radius(0.0), None);
    assert_eq!(direct_radius(f32::NAN), None);
    assert_eq!(margin(0.6), 2.0);
    // The cascade keeps its own spread for large radii.
    assert_eq!(margin(28.0), 48.0);
}

#[test]
fn weights_are_a_normalized_symmetric_gaussian() {
    for sigma in [0.3f32, 0.6, 1.0, 2.5, 4.0] {
        let r = direct_radius(sigma).unwrap();
        let w = weights(sigma, r);
        let sum: f32 = w[0] + 2.0 * w[1..=r].iter().sum::<f32>();
        assert!((sum - 1.0).abs() < 1e-6, "σ {sigma}: {sum}");
        assert!(w[1..=r].windows(2).all(|p| p[0] > p[1]));
        assert!(w[r + 1..].iter().all(|v| *v == 0.0));
        // Ratio of neighbours is the Gaussian's exp(-(2k+1)/(2σ²)).
        let expect = (-1.0 / (2.0 * sigma * sigma)).exp();
        assert!((w[1] / w[0] - expect).abs() < 1e-5);
    }
}
