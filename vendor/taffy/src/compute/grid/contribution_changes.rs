//! Повторная оценка вкладов сохраняет кэши и порядок всех элементов оси.
use super::types::{GridItem, GridTrack};
use crate::geometry::{AbstractAxis, Size};
use crate::tree::{LayoutPartialTree, LayoutPartialTreeExt};

/// Отдельное продолжение не включает измеритель элемента в кадр всей сетки.
pub(super) async fn recompute(
    tree: &mut impl LayoutPartialTree,
    items: &mut [GridItem],
    axis: AbstractAxis,
    tracks: &[GridTrack],
    other_tracks: &[GridTrack],
    inner_size: Size<Option<f32>>,
    stack: &mut reblessive::Stk,
) -> bool {
    let mut any_changed = false;
    for item in items
        .iter_mut()
        .filter(|item| item.crosses_intrinsic_track(axis) && item.sizes_axis(axis))
    {
        let grid_area_size = item.grid_area_size(
            axis,
            tracks,
            other_tracks,
            inner_size,
            |track: &GridTrack, _| Some(track.base_size),
            &|val, basis| tree.calc(val, basis),
        );
        let available_space = grid_area_size.with(axis, None);
        let contribution = item
            .min_content_contribution_async(axis, tree, grid_area_size, available_space, stack)
            .await;
        any_changed |= Some(contribution) != item.min_content_contribution_cache.get(axis);
        item.grid_area_size_cache = Some(grid_area_size);
        item.min_content_contribution_cache
            .set(axis, Some(contribution));
        item.max_content_contribution_cache.set(axis, None);
        item.minimum_contribution_cache.set(axis, None);
    }
    any_changed
}
