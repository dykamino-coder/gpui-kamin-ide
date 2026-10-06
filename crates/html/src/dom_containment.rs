//! Containment eligibility is determined by the principal box after blockification.

use crate::computed::{Computed, Display, RubyRole};

pub(super) fn normalize(style: &mut Computed, tag: &str, out_of_flow: bool) {
    // CSS Containment 2 §§3.1–3.4: display:contents has no principal box.
    let no_box = matches!(style.display, Some(Display::Contents | Display::None))
        && style.col_role.is_none();
    let atomic_by_tag = matches!(
        tag,
        "img"
            | "svg"
            | "canvas"
            | "video"
            | "audio"
            | "embed"
            | "object"
            | "iframe"
            | "input"
            | "select"
            | "textarea"
            | "button"
            | "meter"
            | "progress"
    );
    let non_atomic_inline = !out_of_flow
        && !atomic_by_tag
        && (style.inline_display == Some(true)
            || (style.display.is_none() && !super::BLOCK_TAGS.contains(&tag)));
    let internal_ruby = !out_of_flow && style.ruby_role.is_some_and(|r| r != RubyRole::Container);
    let internal_table = matches!(
        style.display,
        Some(Display::TableCell | Display::TableRow | Display::TableRowGroup)
    ) || style.col_role.is_some();
    // Size containment excludes all internal table boxes, including cells.
    if no_box || non_atomic_inline || internal_ruby || internal_table {
        style.contain_size = None;
        style.contain_inline_size = None;
    }
    // Layout/paint containment may apply to cells, unlike other internal boxes.
    if no_box
        || non_atomic_inline
        || internal_ruby
        || (internal_table && style.display != Some(Display::TableCell))
    {
        style.contain_layout = None;
        style.contain_paint = None;
    }
    if no_box {
        style.contain_style = None;
    }
    // CSS Containment 2 §4: content-visibility cannot skip contents of a
    // boxless element or a non-atomic inline/internal ruby box.
    if no_box || non_atomic_inline || internal_ruby {
        style.skip_content = None;
    }
}
