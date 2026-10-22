//! Линейки промежутков flex-контейнера.
use crate::style::computed::Computed;

/// Линейки промежутков у flex-контейнера (css-gaps-1 `column-rule`/`row-rule`).
pub(crate) fn flex_gap_rules(s: &Computed) -> bool {
    s.column_rule_visible == Some(true)
        || s.row_rule_visible == Some(true)
        || s.column_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
        || s.row_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
}
