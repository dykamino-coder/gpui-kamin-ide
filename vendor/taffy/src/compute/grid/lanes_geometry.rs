//! Grid-lanes alignment uses real track edges and clamps safe overflow before reversal.
use super::types::GridTrack;
use crate::geometry::{Rect, Size};
use crate::{AlignContent, AlignContentKeyword, AlignmentSafety, Line};

/// Gutters represent grid lines, but their offsets include the preceding gap.
/// Absolute areas begin at the next real track and end at the previous real track.
pub(super) fn absolute_area(
    tracks: &[GridTrack],
    placement: Line<Option<usize>>,
    padding_start: f32,
    padding_end: f32,
    reversed: bool,
) -> Line<f32> {
    if reversed {
        let start = placement
            .end
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| tracks.get(index))
            .map(|track| track.offset)
            .unwrap_or(padding_start);
        let end = placement
            .start
            .and_then(|index| tracks.get(index))
            .map(|track| track.offset)
            .unwrap_or(padding_end);
        return Line {
            start: start.min(end),
            end: start.max(end),
        };
    }
    let start = placement
        .start
        .and_then(|index| {
            tracks.get(index + 1).map(|track| track.offset).or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(|i| tracks.get(i))
                    .map(|track| track.offset + track.base_size)
            })
        })
        .unwrap_or(padding_start);
    let end = placement
        .end
        .and_then(|index| {
            index
                .checked_sub(1)
                .and_then(|i| tracks.get(i))
                .map(|track| track.offset + track.base_size)
                .or_else(|| tracks.get(1).map(|track| track.offset))
        })
        .unwrap_or(padding_end);
    Line {
        start: start.min(end),
        end: start.max(end),
    }
}

/// KaminIDE patch: Compute the physical area covered by a lane range.
pub(super) fn track_area(
    tracks: &[GridTrack],
    start: usize,
    end: usize,
    reversed: bool,
) -> Line<f32> {
    if reversed {
        Line {
            start: tracks[2 * end - 1].offset,
            end: tracks[2 * start].offset,
        }
    } else {
        Line {
            start: tracks[2 * start + 1].offset,
            end: tracks[2 * end].offset,
        }
    }
}

/// Overflow safety clamps the distance, including the distance used to reverse placement.
pub(super) fn stacking_alignment_offset(
    free: f32,
    alignment: AlignContent,
    fill_reverse: bool,
) -> f32 {
    let free = if alignment.safety == AlignmentSafety::Safe {
        free.max(0.0)
    } else {
        free
    };
    let keyword = crate::compute::common::alignment::apply_alignment_fallback(free, 1, alignment);
    let offset = match keyword {
        AlignContentKeyword::Center => free / 2.0,
        AlignContentKeyword::End | AlignContentKeyword::FlexEnd => free,
        _ => 0.0,
    };
    if fill_reverse {
        offset - free
    } else {
        offset
    }
}

/// The in-flow grid area contributes overflow even when all its lanes are empty.
/// Track coordinates include borders; overflow is relative to the padding box.
#[cfg(feature = "content_size")]
pub(super) fn inflow_bounds(
    tracks: &[GridTrack],
    count: usize,
    reversed: bool,
    rows: bool,
    stack_start: f32,
    stack_size: f32,
    border: Rect<f32>,
) -> Rect<f32> {
    if count == 0 {
        return Rect::ZERO;
    }
    let grid = track_area(tracks, 0, count, reversed);
    let stack_end = stack_start + stack_size;
    if rows {
        Rect {
            left: stack_start - border.left,
            right: stack_end - border.left,
            top: grid.start - border.top,
            bottom: grid.end - border.top,
        }
    } else {
        Rect {
            left: grid.start - border.left,
            right: grid.end - border.left,
            top: stack_start - border.top,
            bottom: stack_end - border.top,
        }
    }
}

/// Grid tracks contribute in-flow overflow independently of their item count.
/// An empty axis has a single line at its aligned start, rather than a track.
pub(super) fn grid_inflow_bounds(
    columns: &[GridTrack],
    rows: &[GridTrack],
    reversed: Size<bool>,
    border: Rect<f32>,
) -> Rect<f32> {
    #[cfg(feature = "content_size")]
    {
        let axis = |tracks: &[GridTrack], reverse| {
            if tracks.len() > 1 {
                track_area(tracks, 0, tracks.len() / 2, reverse)
            } else {
                let offset = tracks.first().map_or(0.0, |track| track.offset);
                Line {
                    start: offset,
                    end: offset,
                }
            }
        };
        let x = axis(columns, reversed.width);
        let y = axis(rows, reversed.height);
        Rect {
            left: x.start - border.left,
            right: x.end - border.left,
            top: y.start - border.top,
            bottom: y.end - border.top,
        }
    }
    #[cfg(not(feature = "content_size"))]
    {
        let _ = (columns, rows, reversed, border);
        Rect::ZERO
    }
}
