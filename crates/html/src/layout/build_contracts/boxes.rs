//! Box requests of the build contract: the styled div of an element, a bare
//! div under computed values and the content-size wrapper. Layout decides
//! which box it needs and under which style; the adapter from computed
//! values to `gpui::Style` (paint's `box_tree` and `apply`) builds it.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::cx::LayoutCx;
use crate::style::computed::{Align, BoxStyle, Sides};
use crate::style::values::value::Len;
use gpui::AnyElement;

/// Where the wrapped item sits in its parent's grid: line placement and
/// line names.
pub(crate) type GridPlacement = (Option<gpui::GridLocation>, Option<gpui::GridLineNames>);

type StyledBox = fn(&Element, &BoxStyle, &LayoutCx, Option<BoxDimensions>) -> gpui::Div;
type StyledDiv = fn(Styling<'_>, &LayoutCx) -> gpui::Div;
type ContentSized = fn(AnyElement, &BoxStyle, &BoxStyle, GridPlacement, &LayoutCx) -> AnyElement;

/// The computed values a bare div (one without an element) is styled with.
pub(crate) enum Styling<'a> {
    /// Every box value of the style: display, sizes, edges, paint.
    Box(&'a BoxStyle),
    /// Only the text values of the style: font, color, decoration, line
    /// height, alignment and spacing.
    Text(&'a BoxStyle),
    /// Only the margins (`auto` kept as an auto margin).
    Margins(&'a Sides),
}

/// The box operations of [`ChildBuild`](super::ChildBuild).
#[derive(Clone, Copy)]
pub(crate) struct BoxBuild {
    styled_box: StyledBox,
    styled_div: StyledDiv,
    content_sized: ContentSized,
    content_for_element:
        fn(AnyElement, &Element, &BoxStyle, GridPlacement, ValueCx<'_>) -> AnyElement,
    length: fn(Len, ValueCx<'_>) -> gpui::DefiniteLength,
    alignment: fn(Align, bool) -> gpui::AlignItems,
}

impl BoxBuild {
    pub(crate) const fn new(
        styled_box: StyledBox,
        styled_div: StyledDiv,
        content_sized: ContentSized,
        content_for_element: fn(
            AnyElement,
            &Element,
            &BoxStyle,
            GridPlacement,
            ValueCx<'_>,
        ) -> AnyElement,
        length: fn(Len, ValueCx<'_>) -> gpui::DefiniteLength,
        alignment: fn(Align, bool) -> gpui::AlignItems,
    ) -> Self {
        Self {
            styled_box,
            styled_div,
            content_sized,
            content_for_element,
            length,
            alignment,
        }
    }

    pub(super) fn content_for_element(
        &self,
        el: AnyElement,
        e: &Element,
        parent: &BoxStyle,
        placement: GridPlacement,
        vcx: ValueCx<'_>,
    ) -> AnyElement {
        (self.content_for_element)(el, e, parent, placement, vcx)
    }

    pub(super) fn length(&self, length: Len, vcx: ValueCx<'_>) -> gpui::DefiniteLength {
        (self.length)(length, vcx)
    }

    pub(super) fn alignment(&self, align: Align, last: bool) -> gpui::AlignItems {
        (self.alignment)(align, last)
    }

    pub(super) fn styled_box(&self, e: &Element, style: &BoxStyle, cx: &LayoutCx) -> gpui::Div {
        (self.styled_box)(e, style, cx, None)
    }

    pub(super) fn sized_box(
        &self,
        e: &Element,
        style: &BoxStyle,
        cx: &LayoutCx,
        sizes: BoxDimensions,
    ) -> gpui::Div {
        (self.styled_box)(e, style, cx, Some(sizes))
    }

    pub(super) fn styled_div(&self, styling: Styling<'_>, cx: &LayoutCx) -> gpui::Div {
        (self.styled_div)(styling, cx)
    }

    pub(super) fn content_sized(
        &self,
        el: AnyElement,
        style: &BoxStyle,
        item_style: &BoxStyle,
        placement: GridPlacement,
        cx: &LayoutCx,
    ) -> AnyElement {
        (self.content_sized)(el, style, item_style, placement, cx)
    }
}
