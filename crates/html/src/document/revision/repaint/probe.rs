//! Targets skipped by traversal and their DevTools refusal context.

use crate::paint_path as why;
use markup5ever_rcdom::Handle;

/// Two absent boxes need no update. Hidden html is an exception: traversal
/// still creates its empty box with computed style.
/// Outer None means refusal; inner None means a target skipped on both sides.
pub(super) fn pair<T>(target: &Handle, old: Option<T>, new: Option<T>) -> Option<Option<(T, T)>> {
    match (old, new) {
        (Some(old), Some(new)) => Some(Some((old, new))),
        (None, None) if super::super::adopt::tag_of(target) != "html" => Some(None),
        (old, new) => {
            why::refuse_with(&why::REFUSED_RESTYLE_PROBE, || {
                format!(
                    "node={} tag={} old={} new={}",
                    target.key().packed(),
                    super::super::adopt::tag_of(target),
                    if old.is_some() { "present" } else { "none" },
                    if new.is_some() { "present" } else { "none" },
                )
            });
            None
        }
    }
}
