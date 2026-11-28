//! Which condition of an inert box of a kept paragraph (`paragraph::key::inert`) a
//! box fails, by name: the `why` journal of a refused paragraph names it
//! (`paragraph.subtree`).

use crate::dom::Element;

/// The first element of the subtree of `e` that is not inert, and the first
/// condition it fails; `None` if all are inert.
pub(in crate::layout::reuse) fn not_inert(
    e: &Element,
    vcx: crate::values::ValueCx<'_>,
) -> Option<(String, &'static str)> {
    let mut found = None;
    super::all_elements(e, |e| {
        let why = reason(e, vcx);
        if let Some(why) = why {
            found = Some((e.tag.to_string(), why));
        }
        why.is_none()
    });
    found
}

fn reason(e: &Element, vcx: crate::values::ValueCx<'_>) -> Option<&'static str> {
    let style = e.style.used();
    let positioned = &style.reset_positioned;
    match () {
        _ if super::embedded(e) => Some("embedded"),
        _ if super::math_layout(e) => Some("math"),
        _ if crate::text::ruby::ruby_role(e).is_some() => Some("ruby"),
        _ if e.anim.is_some() => Some("animation"),
        _ if e.first_letter.is_some() || e.first_line.is_some() => Some("first_line_or_letter"),
        _ if e.list_item.is_some() => Some("list_item"),
        _ if !super::unlayered(e) => Some("layered"),
        _ if style.wm().vertical() => Some("vertical"),
        _ if style.has_viewport_lengths(vcx) => Some("viewport_lengths"),
        _ if !super::layout_reads_outside_nothing(e, vcx) => Some("flex_or_grid"),
        _ if positioned.anchor_name.is_some() => Some("anchor_name"),
        _ if positioned.implicit_anchor.is_some()
            && crate::layout::positioned::anchor::wants_implicit(&style) =>
        {
            Some("implicit_anchor")
        }
        _ => None,
    }
}
