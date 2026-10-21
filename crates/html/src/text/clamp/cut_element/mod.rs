//! Чистая геометрия clamp и проверка повторной сборки в WPT-harness.

mod painting;
use super::ClampEntry;
pub(crate) use painting::ClampGeometry;

/// Прежний потребитель paint больше не принимает решений о раскладке.
#[cfg(feature = "wpt-harness")]
pub(crate) struct ClampCut;

#[cfg(feature = "wpt-harness")]
impl ClampCut {
    pub(crate) fn check(expected: &super::ClampPlan, actual: &super::ClampPlan) {
        debug_assert_eq!(
            expected, actual,
            "clamp builder must reproduce the measured plan"
        );
    }
}
