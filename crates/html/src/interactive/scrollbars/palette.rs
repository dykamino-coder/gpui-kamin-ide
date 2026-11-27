//! Native automatic scrollbar colors for the element's used color scheme.
//! CSS Color Adjustment 1 section 2.2 includes scrollbars in scheme adjustment;
//! CSS Scrollbars 1 leaves the native palette to the UA. These are the Windows
//! Fluent normal-state track and thumb colors, independent of document content.

pub(super) fn automatic(dark: bool) -> (u32, u32) {
    if dark { (44, 159) } else { (252, 139) }
}
