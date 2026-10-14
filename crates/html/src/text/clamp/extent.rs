//! Clamp point by box-tree order: content before the point keeps its overflow.

use super::{clamp_cut, clamp_extent, clamp_para};

/// The visible extent of the content before the clamp point reaches below
/// the cut (css-overflow-4 §5.3: only boxes and lines AFTER the clamp point
/// are hidden; Blink marks them hidden for paint instead of clipping the
/// container). Returns that extent from the content top.
pub fn revealed_extent(key: u64) -> Option<f32> {
    let cut = clamp_cut(key).filter(|c| c.is_finite())?;
    clamp_extent(key).filter(|e| *e > cut + 0.5)
}

/// Extend the container's clip edge down to the revealed extent; `cut` is
/// the used content height and `pad_bottom` the bottom padding (the clip
/// edge is the padding box).
pub fn extend_clip(style: &mut gpui::StyleRefinement, key: u64, cut: f32, pad_bottom: f32) {
    if let Some(ext) = revealed_extent(key) {
        style.overflow_clip_offset = Some([0.0, 0.0, ext - cut - pad_bottom, 0.0]);
    }
}

/// A paragraph numbered after the clamp paragraph follows the clamp point:
/// hidden for paint once the clip no longer hides it.
pub fn hidden_after_point(key: u64, seq: u32) -> bool {
    revealed_extent(key).is_some() && clamp_para(key).is_some_and(|(s, _)| seq > s)
}

/// An in-flow block box whose paragraphs all follow the clamp point is
/// hidden for paint with them (css-overflow-4 §5.3; Blink hides the boxes
/// after the clamp point instead of clipping): once the clip reveals the
/// overflow of earlier content, its border and background must not show
/// (`line-clamp-auto-047`). Floats, positioned and atomic inline boxes keep
/// the clip.
pub fn hide_box_after_point(
    mut d: gpui::Div,
    outer: Option<(u64, bool)>,
    span: Option<super::ClampSpan>,
    style: &crate::style::computed::Computed,
    is_clamp: bool,
) -> gpui::Div {
    use crate::style::computed::{Display, Position};
    use gpui::Styled;
    super::close_clamp_span(span.clone());
    let first = span.as_ref().map(|s| s.get().0);
    let in_flow = !style.float.is_some_and(|f| f != 0)
        && !matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
        && !matches!(
            style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        );
    if let (Some((key, false)), Some(first)) = (outer, first)
        && !is_clamp
        && in_flow
        && hidden_after_point(key, first)
    {
        d.style().visibility = Some(gpui::Visibility::Hidden);
    }
    d
}
