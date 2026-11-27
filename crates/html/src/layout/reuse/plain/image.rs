//! Images a box's content replays while the image store keeps their state.

use crate::dom::Element;

/// An `<img>` that is inert but for its pixels, which the image store holds:
/// a box's content replays it while its request keeps its state
/// (`ImageStore::replay_reads`). One choosing a candidate by the viewport or
/// the device scale (`srcset`, `sizes`, HTML §4.8.4.3) stays out.
pub(in crate::layout::reuse) fn store_image(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    e.tag.as_str() == "img"
        && e.attr("srcset").is_none()
        && e.attr("sizes").is_none()
        && super::own_state_free(e)
        && super::unlayered(e)
        && super::layout_reads_outside_nothing(e, vcx)
}
