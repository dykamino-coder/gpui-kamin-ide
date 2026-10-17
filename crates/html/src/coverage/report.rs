//! CSS coverage report checks.

use super::*;

#[test]
fn coverage_percentage_is_finite_and_bounded() {
    assert!(!PROPERTIES.is_empty());
    assert!(mapped_pct().is_finite());
    assert!((0.0..=100.0).contains(&mapped_pct()));
}
