//! Subgrid floors phase. CSS Grid 2 §9.

use super::super::types::GridItem;
use super::super::types::GridTrack;
use super::super::types::TrackCounts;
use super::SubgridEdge;
use crate::geometry::AbstractAxis;
use crate::tree::NodeId;

/// Пол дорожек лунок от АВТО-размещённых подсеток (Blink
/// `AccommodateSubgridExtraMargins`, ветка `is_auto_placed`, и
/// `LargestAutoPlacedSubgridContribution`, `grid_layout_utils.cc:1236-1258`;
/// резолюция csswg-drafts#10926): позиция такой подсетки при размере дорожек
/// не известна, и её наибольший вклад краёв в одну дорожку получает КАЖДАЯ
/// дорожка — у подсетки в одну дорожку оба края, в две — край и половина
/// разницы зазоров, шире — ещё и целая разница у внутренних
/// (`column-subgrid-extra-margin-006..009`: четыре колонки по 29).
pub(in super::super) fn apply_lanes_auto_floors(
    tracks: &mut [GridTrack],
    edges: &[SubgridEdge],
    columns: bool,
    auto_root: impl Fn(NodeId) -> bool,
) {
    let mut largest = 0.0f32;
    for edge in edges
        .iter()
        .filter(|e| e.columns == columns && auto_root(e.root))
    {
        let half = edge.delta / 2.0;
        let contribution = match edge.span.span() {
            0 | 1 => edge.start + edge.end,
            2 => (edge.start + half).max(edge.end + half),
            _ => (edge.start + half).max(edge.delta).max(edge.end + half),
        };
        largest = largest.max(contribution);
    }
    if largest <= 0.0 {
        return;
    }
    for (i, track) in tracks.iter_mut().enumerate() {
        if i % 2 == 1 && !track.is_collapsed {
            track.subgrid_floor = track.subgrid_floor.max(largest);
        }
    }
}

/// Пол дорожек оси от краёв подсеток (Blink `AccommodateSubgridExtraMargins`,
/// `grid_layout_utils.cc:1260-1356`): у подсетки с определённой позицией
/// крайняя дорожка пролёта получает свой накопленный край, а при пролёте в
/// одну дорожку — сумму обоих. Так пустая подсетка с полями и отбивками
/// (`subgrid-no-items-on-edges-001`) всё равно раздвигает дорожки родителя.
pub(in super::super) fn apply_subgrid_floors(
    tracks: &mut [GridTrack],
    edges: &[SubgridEdge],
    columns: bool,
    counts: TrackCounts,
) {
    for track in tracks.iter_mut() {
        track.subgrid_floor = 0.0;
    }
    for edge in edges.iter().filter(|e| e.columns == columns) {
        let (Some(start), Some(end)) = (
            edge.span.start.try_into_track_vec_index(counts),
            edge.span.end.try_into_track_vec_index(counts),
        ) else {
            continue;
        };
        if end <= start {
            continue;
        }
        let (first, last) = (start + 1, end - 1);
        let mut floor = |i: usize, v: f32| {
            if let Some(track) = tracks.get_mut(i) {
                track.subgrid_floor = track.subgrid_floor.max(v);
            }
        };
        if first == last {
            floor(first, edge.start + edge.end);
        } else {
            floor(first, edge.start);
            floor(last, edge.end);
        }
    }
}

/// Переставить элементы так, чтобы вкладывающиеся в ось `axis` стояли
/// первыми, и вернуть их число: алгоритм размеров дорожек получает срез
/// `items[..k]`. Перестановка устойчивая; исходный порядок восстанавливает
/// сортировка по `source_order` перед раскладкой.
pub(in super::super) fn partition_for_axis(items: &mut [GridItem], axis: AbstractAxis) -> usize {
    if items.iter().all(|item| item.sizes_axis(axis)) {
        return items.len();
    }
    items.sort_by_key(|item| !item.sizes_axis(axis));
    items
        .iter()
        .take_while(|item| item.sizes_axis(axis))
        .count()
}
