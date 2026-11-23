//! DevTools context for a target the restyle walk could not enter.

use crate::paint_path as why;
use markup5ever_rcdom::Handle;

/// Keep the refusal at the target, with both sides of the attempted probe.
pub(super) fn pair<T>(target: &Handle, old: Option<T>, new: Option<T>) -> Option<(T, T)> {
    match (old, new) {
        (Some(old), Some(new)) => Some((old, new)),
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
