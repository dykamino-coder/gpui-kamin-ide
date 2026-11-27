//! The `html`/`body` wrapper of the root flow reads nothing of its edge children.

use crate::dom::Element;

/// The `html`/`body` wrapper (or none) reads no baselines or lines of the
/// flow's edge children: a block box without first-line and first-letter
/// pseudo-elements, without text-edge trimming (css-inline-3 §5) and without
/// a content clip, under which a replayed paint would lie under another mask.
pub(in crate::layout::reuse) fn edges_free(e: Option<&Element>) -> bool {
    use crate::style::computed::{Display, Overflow};
    let Some(e) = e else { return true };
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let visible = |o: Option<Overflow>| matches!(o, None | Some(Overflow::Visible));
    e.first_line.is_none()
        && e.first_letter.is_none()
        && matches!(b.display, None | Some(Display::Block))
        && visible(b.overflow_x)
        && visible(b.overflow_y)
        && !style.reset_text_box.text_box_trim_start
        && !style.reset_text_box.text_box_trim_end
}
