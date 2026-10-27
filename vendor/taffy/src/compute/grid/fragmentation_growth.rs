//! KaminIDE patch: block space that a fragmenting caller inserts after grid tracks
//! (css-grid-2 §12 Fragmenting Grid Layout: a row pushed into the next fragmentainer moves with
//! every following row; Blink `ShiftBreakpointIntoNextFragmentainer`, Gecko continuation row
//! offsets). Track sizing and content alignment keep the unfragmented layout; the growth only
//! moves the tracks after it towards the axis end, so an item spanning it grows with its grid
//! area and a container sized by its content grows within its min/max sizes. A subgridded axis
//! has no own tracks (css-grid-2 §9): it takes the parent's growth inside its span, so its
//! lines stay on the parent's lines, and ignores the caller's own request.
use super::types::{GridItem, GridTrack, GridTrackKind};
use crate::geometry::{AbsoluteAxis, Line, Size};
use crate::style::AvailableSpace;
use crate::tree::NodeId;
use crate::util::sys::{f32_max, Vec};
use crate::util::MaybeMath;
use crate::LayoutGridContainer;

/// Size constraints of the container that decide whether growth may enlarge it.
pub(super) struct Limits {
    /// Known or preferred border-box size: such an axis keeps its size.
    pub(super) resolved: Size<Option<f32>>,
    /// Authored preferred border-box size: its content alignment uses the full content box.
    pub(super) preferred: Size<Option<f32>>,
    pub(super) inset: Size<f32>,
    pub(super) min: Size<Option<f32>>,
    pub(super) max: Size<Option<f32>>,
    pub(super) padding_border: Size<f32>,
}

/// Growth after each track of both physical axes, in exported track order.
pub(super) struct Growth {
    after: Size<Vec<f32>>,
    total: Size<f32>,
}

impl Growth {
    /// Read the growth for the tracks of this grid: the caller's, or in a subgridded axis
    /// (`inherited` is `Some`) the parent's growth inside the span.
    pub(super) fn read(
        tree: &impl LayoutGridContainer,
        node: NodeId,
        columns: &[GridTrack],
        rows: &[GridTrack],
        inherited: Size<Option<&[f32]>>,
    ) -> Self {
        let axis = |abs: AbsoluteAxis, tracks: &[GridTrack], inherited: Option<&[f32]>| -> Vec<f32> {
            let count = tracks.iter().filter(|track| track.kind == GridTrackKind::Track).count();
            nonzero(
                (0..count)
                    .map(|index| match inherited {
                        Some(growth) => growth.get(index).copied().unwrap_or(0.0),
                        None => tree.grid_track_fragmentation_growth(node, abs, index),
                    })
                    .map(|value| if value.is_finite() && value > 0.0 { value } else { 0.0 })
                    .collect(),
            )
        };
        let after = Size {
            width: axis(AbsoluteAxis::Horizontal, columns, inherited.width),
            height: axis(AbsoluteAxis::Vertical, rows, inherited.height),
        };
        let total = Size { width: after.width.iter().sum(), height: after.height.iter().sum() };
        Self { after, total }
    }

    /// Content-box size for the unfragmented track sizing: a size a parent `given` this
    /// container without an authored size was measured with the growth and excludes it here.
    pub(super) fn unfragmented_size(&self, size: Size<Option<f32>>, given: Size<bool>) -> Size<Option<f32>> {
        let less = |size: Option<f32>, given: bool, total: f32| match size {
            Some(size) if given && total > 0.0 => Some(f32_max(size - total, 0.0)),
            size => size,
        };
        Size {
            width: less(size.width, given.width, self.total.width),
            height: less(size.height, given.height, self.total.height),
        }
    }

    /// Available grid space for the unfragmented track sizing (see `unfragmented_size`).
    pub(super) fn unfragmented_space(&self, space: Size<AvailableSpace>, given: Size<bool>) -> Size<AvailableSpace> {
        let less = |space: AvailableSpace, given: bool, total: f32| match space {
            AvailableSpace::Definite(size) if given && total > 0.0 => AvailableSpace::Definite(f32_max(size - total, 0.0)),
            space => space,
        };
        Size {
            width: less(space.width, given.width, self.total.width),
            height: less(space.height, given.height, self.total.height),
        }
    }

    /// No growth: the layout is exactly the unfragmented one.
    pub(super) fn is_empty(&self) -> bool {
        self.after.width.is_empty() && self.after.height.is_empty()
    }

    /// Border-box size of the container: an axis sized by its tracks grows by the total growth.
    pub(super) fn container(&self, size: Size<f32>, track_sums: Size<f32>, limits: &Limits) -> Size<f32> {
        let grow = |size: f32, total: f32, resolved: Option<f32>, sum: f32, inset: f32, min, max, floor: f32| {
            if total <= 0.0 || resolved.is_some() {
                return size;
            }
            (sum + total + inset).maybe_clamp(min, max).max(floor).max(size)
        };
        Size {
            width: grow(
                size.width,
                self.total.width,
                limits.resolved.width,
                track_sums.width,
                limits.inset.width,
                limits.min.width,
                limits.max.width,
                limits.padding_border.width,
            ),
            height: grow(
                size.height,
                self.total.height,
                limits.resolved.height,
                track_sums.height,
                limits.inset.height,
                limits.min.height,
                limits.max.height,
                limits.padding_border.height,
            ),
        }
    }

    /// Content size that content alignment distributes: that of the unfragmented layout.
    /// An authored size keeps its free space; a size taken from the content (auto, or given by
    /// a parent that measured this grid with its growth) excludes the growth.
    pub(super) fn alignment_basis(&self, content: Size<f32>, track_sums: Size<f32>, limits: &Limits) -> Size<f32> {
        let basis = |content: f32, total: f32, preferred: Option<f32>, sum: f32, inset: f32, min, max, floor: f32| {
            if total <= 0.0 || preferred.is_some() {
                return content;
            }
            let unfragmented = (sum + inset).maybe_clamp(min, max).max(floor) - inset;
            unfragmented.max(content - total).min(content).max(0.0)
        };
        Size {
            width: basis(
                content.width,
                self.total.width,
                limits.preferred.width,
                track_sums.width,
                limits.inset.width,
                limits.min.width,
                limits.max.width,
                limits.padding_border.width,
            ),
            height: basis(
                content.height,
                self.total.height,
                limits.preferred.height,
                track_sums.height,
                limits.inset.height,
                limits.min.height,
                limits.max.height,
                limits.padding_border.height,
            ),
        }
    }

    /// Move the aligned tracks. `extra` is how much larger the content box is than the
    /// alignment basis: on a reversed axis the axis start is the high physical edge, so the
    /// tracks before every growth move with it.
    pub(super) fn shift(&self, columns: &mut [GridTrack], rows: &mut [GridTrack], reversed: Size<bool>, extra: Size<f32>) {
        shift_axis(columns, &self.after.width, reversed.width, extra.width);
        shift_axis(rows, &self.after.height, reversed.height, extra.height);
    }

    /// Hand each subgrid the growth between the tracks it spans, in its own track order. The
    /// tracks published to it before carry the unfragmented sizes.
    pub(super) fn publish_to_subgrids(&self, tree: &mut impl LayoutGridContainer, items: &[GridItem], parent: Size<bool>) {
        if self.is_empty() {
            return;
        }
        for item in items.iter().filter(|item| !item.flattened) {
            let Some(mut tracks) = tree.get_subgrid_tracks(item.node) else {
                continue;
            };
            let child = super::subgrid_flow::axes(&tree.get_grid_container_style(item.node));
            let mut changed = false;
            if let Some(axis) = tracks.columns.as_mut() {
                changed |= span_growth(&mut axis.growth, &self.after.width, item.column_indexes, parent.width != child.width);
            }
            if let Some(axis) = tracks.rows.as_mut() {
                changed |= span_growth(&mut axis.growth, &self.after.height, item.row_indexes, parent.height != child.height);
            }
            if changed {
                tree.set_subgrid_tracks(item.node, Some(tracks));
            }
        }
    }
}

/// Growth between the tracks of a span `lines` (indices of the track vector, lines at even
/// indices); reversed when the subgrid orders the axis the other way.
fn span_growth(out: &mut Vec<f32>, after: &[f32], lines: Line<u16>, reversed: bool) -> bool {
    let first = usize::from(lines.start / 2);
    let count = usize::from(lines.end.saturating_sub(lines.start) / 2);
    let mut growth: Vec<f32> =
        (first..first + count.saturating_sub(1)).map(|track| after.get(track).copied().unwrap_or(0.0)).collect();
    if reversed {
        growth.reverse();
    }
    let growth = nonzero(growth);
    let changed = *out != growth;
    *out = growth;
    changed
}

/// Growth that inserts nothing is no growth at all.
fn nonzero(growth: Vec<f32>) -> Vec<f32> {
    if growth.iter().any(|&value| value > 0.0) {
        growth
    } else {
        Vec::new()
    }
}

fn shift_axis(tracks: &mut [GridTrack], after: &[f32], reversed: bool, extra: f32) {
    if after.is_empty() {
        return;
    }
    let mut before = 0.0;
    let mut index = 0;
    for track in tracks.iter_mut() {
        track.offset += if reversed { extra - before } else { before };
        if track.kind == GridTrackKind::Track {
            before += after.get(index).copied().unwrap_or(0.0);
            index += 1;
        }
    }
}
