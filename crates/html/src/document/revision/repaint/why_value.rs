//! The value behind an element's damage, for the DevTools `why` journal: the
//! first difference of the named style group of the old and new styles, so a
//! journal names the field that set the level, not only its group.

use crate::style::computed::Computed;

/// Journal the first difference of the group `group` (a name from
/// `Computed::damage`) between `old` and `new`, if the journal is on.
pub(super) fn note(group: &'static str, old: &Computed, new: &Computed) {
    if !crate::why::enabled() {
        return;
    }
    crate::why::note("damage.value", || {
        let (a, b) = (debug(old, group), debug(new, group));
        format!("{group}: {}", first_difference(&a, &b))
    });
}

fn debug(c: &Computed, group: &str) -> String {
    let i = &c.inh.0;
    match group {
        "box_layout" | "box_layout.boxes" => format!("{:?}", c.reset_box_layout),
        "flex" | "flex.order" => format!("{:?}", c.reset_flex),
        "border" => format!("{:?}", c.reset_border),
        "background" => format!("{:?}", c.reset_background),
        "cascade" => format!("{:?}", c.reset_cascade),
        "text_box" => format!("{:?}", c.reset_text_box),
        "inh.font" => format!("{:?}", i.font),
        "inh.text_flow" => format!("{:?}", i.text_flow),
        "inh.text_ink" => format!("{:?}", i.text_ink),
        "inh.writing" => format!("{:?}", i.writing),
        _ => format!("{c:?}"),
    }
}

/// About 60 characters of each side from just before their first difference.
fn first_difference(a: &str, b: &str) -> String {
    let at = a
        .char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map_or(a.len().min(b.len()), |((i, _), _)| i);
    let from = |s: &str| {
        let mut start = at.saturating_sub(24).min(s.len());
        while !s.is_char_boundary(start) {
            start -= 1;
        }
        s[start..].chars().take(60).collect::<String>()
    };
    format!("{} -> {}", from(a), from(b))
}
