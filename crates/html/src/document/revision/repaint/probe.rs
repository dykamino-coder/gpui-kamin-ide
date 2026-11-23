//! Пропущенные обходом цели и контекст отказа DevTools.

use crate::paint_path as why;
use markup5ever_rcdom::Handle;

/// Две отсутствующие коробки не требуют обновления. Исключение — скрытый
/// html: обход всё равно создаёт его пустую коробку с вычисленным стилем.
/// Внешний None означает отказ, внутренний — пропущенную с обеих сторон цель.
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
