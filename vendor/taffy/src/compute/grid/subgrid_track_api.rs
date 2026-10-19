//! Subgrid track api phase. CSS Grid 2 §9.

use super::super::types::GridTrack;
use super::bounded_subgrid_count;
use crate::style::MaxTrackSizingFunction;
use crate::style::MinTrackSizingFunction;
use crate::style::SubgridAxisTracks;
use crate::style::SubgridTracks;
use crate::style::SUBGRID_COLUMNS;
use crate::style::SUBGRID_ROWS;
use crate::tree::LayoutPartialTreeExt;
use crate::tree::NodeId;
use crate::util::sys::Vec;
use crate::LayoutGridContainer;
use crate::LengthPercentage;

/// Подсеточные оси узла, выданные родителем; оси без бита в стиле
/// отбрасываются (запись могла остаться от прежнего стиля узла).
pub(in super::super) fn own_subgrid_tracks<Tree: LayoutGridContainer>(
    tree: &Tree,
    node: NodeId,
    bits: u8,
) -> Option<SubgridTracks> {
    if bits & (SUBGRID_COLUMNS | SUBGRID_ROWS) == 0 {
        return None;
    }
    let mut tracks = tree.get_subgrid_tracks(node)?;
    if bits & SUBGRID_COLUMNS == 0 {
        tracks.columns = None;
    }
    if bits & SUBGRID_ROWS == 0 {
        tracks.rows = None;
    }
    (tracks.columns.is_some() || tracks.rows.is_some()).then_some(tracks)
}

/// Дорожки подсеточной оси из выданных размеров: первая и последняя линии
/// схлопнуты (как у `initialize_grid_tracks`), между дорожками — зазор
/// подсетки. Без размеров (родитель ещё не размерил ось) дорожки `auto`.
pub(in super::super) fn initialize_subgrid_tracks(
    tracks: &mut Vec<GridTrack>,
    axis: &SubgridAxisTracks,
) {
    tracks.clear();
    let count = bounded_subgrid_count(axis.count) as usize;
    tracks.reserve(count * 2 + 1);
    let gutter = || GridTrack::gutter(LengthPercentage::length(axis.gap));
    tracks.push(gutter());
    for i in 0..count {
        let track = match &axis.sizes {
            Some(sizes) => {
                let size = sizes.get(i).copied().unwrap_or(0.0);
                GridTrack::new(
                    MinTrackSizingFunction::length(size),
                    MaxTrackSizingFunction::length(size),
                )
            }
            None => GridTrack::new(
                MinTrackSizingFunction::auto(),
                MaxTrackSizingFunction::auto(),
            ),
        };
        tracks.push(track);
        tracks.push(gutter());
    }
    if let Some(first) = tracks.first_mut() {
        first.collapse();
    }
    if let Some(last) = tracks.last_mut() {
        last.collapse();
    }
}

/// Размеры записей вектора дорожек, известные ДО алгоритма дорожек: у
/// записи с одинаковыми определёнными гранями (`100px`, зазор) и у
/// схлопнутой. Подсетку меряют и до размера дорожек родителя (вклад во
/// вторую ось, мерка лунок по оси укладки) — фиксированные дорожки ей
/// отдаются сразу (у прежнего среза `dom.rs` они были всегда).
pub(in super::super) fn fixed_track_sizes<Tree: LayoutGridContainer>(
    tree: &Tree,
    tracks: &[GridTrack],
    basis: Option<f32>,
) -> Vec<Option<f32>> {
    tracks
        .iter()
        .map(|t| {
            if t.is_collapsed {
                return Some(0.0);
            }
            let min = t
                .min_track_sizing_function
                .definite_value(basis, |val, b| tree.calc(val, b));
            let max = t
                .max_track_sizing_function
                .definite_value(basis, |val, b| tree.calc(val, b));
            match (min, max) {
                (Some(a), Some(b)) if (a - b).abs() < 0.001 => Some(a),
                _ => None,
            }
        })
        .collect()
}
