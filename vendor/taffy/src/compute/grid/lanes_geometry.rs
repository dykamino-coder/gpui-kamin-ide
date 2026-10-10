//! Grid-lanes alignment uses real track edges and clamps safe overflow before reversal.
use super::types::GridTrack;
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
