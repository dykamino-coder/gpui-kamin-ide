//! Box requests of the build contract: the styled div of an element and the
//! content-size wrapper. Layout decides which box it needs and under which
//! style; the adapter from computed values to `gpui::Style` (paint's
//! `box_tree` and `apply`) builds it.

use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use crate::style::computed::Computed;
use gpui::AnyElement;

/// Where the wrapped item sits in its parent's grid: line placement and
/// line names.
pub(crate) type GridPlacement = (Option<gpui::GridLocation>, Option<gpui::GridLineNames>);

type StyledBox = fn(&Element, &Computed, &LayoutCx) -> gpui::Div;
type ContentSized = fn(AnyElement, &Computed, &Computed, GridPlacement, &LayoutCx) -> AnyElement;

/// The box operations of [`ChildBuild`](super::ChildBuild).
#[derive(Clone, Copy)]
pub(crate) struct BoxBuild {
    styled_box: StyledBox,
    content_sized: ContentSized,
}

impl BoxBuild {
    pub(crate) const fn new(styled_box: StyledBox, content_sized: ContentSized) -> Self {
        Self {
            styled_box,
            content_sized,
        }
    }

    pub(super) fn styled_box(&self, e: &Element, style: &Computed, cx: &LayoutCx) -> gpui::Div {
        (self.styled_box)(e, style, cx)
    }

    pub(super) fn content_sized(
        &self,
        el: AnyElement,
        style: &Computed,
        item_style: &Computed,
        placement: GridPlacement,
        cx: &LayoutCx,
    ) -> AnyElement {
        (self.content_sized)(el, style, item_style, placement, cx)
    }
}
