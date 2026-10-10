//! Inherit subgrid track sizes in logical order, including reversed physical offsets.
use super::types::GridTrack;
use crate::util::sys::{String, Vec};
use crate::{Line, SubgridAxisTracks};

/// Размеры дорожек одной подсеточной оси из дорожек родителя.
///
/// `lines` — индексы линий области подсетки в векторе дорожек родителя
/// (чётные: линии/зазоры, нечётные: дорожки). Линии подсетки совпадают с
/// линиями родителя: начало первой дорожки — край области плюс край
/// подсетки, внутренняя линия — середина зазора родителя (вместе с долей
/// распределения содержимого) минус/плюс половина своего зазора
/// (css-grid-2 §subgrid-gaps, Note: «the subgrid's gutters will visually
/// center-align with the parent grid's gutters»). Размер бывает и
/// отрицательным (`grid-gap-011-ref`: 25 / −50 / 25; Blink
/// `accumulated_gutter_size_delta_` пола не имеет).
#[allow(clippy::too_many_arguments)]
pub(super) fn axis_tracks(
    tracks: &[GridTrack],
    lines: Line<u16>,
    lead: f32,
    trail: f32,
    gap: f32,
    sized: bool,
    use_offsets: bool,
    fixed: Option<&[Option<f32>]>,
    names: Vec<Vec<String>>,
    parent_reversed: bool,
    child_reversed: bool,
) -> SubgridAxisTracks {
    let (lead, trail) = if parent_reversed {
        (trail, lead)
    } else {
        (lead, trail)
    };
    let count = ((lines.end.saturating_sub(lines.start)) / 2).max(1);
    let (first, last) = (
        lines.start as usize,
        lines.start as usize + 2 * count as usize,
    );
    // Размеры и позиции записей вектора в пределах пролёта (позиции — от
    // его начальной линии). Неразмеренная ось годится, только если ВСЕ
    // записи пролёта фиксированы (`fixed_track_sizes`): тогда их размер
    // известен и до алгоритма дорожек.
    let mut base: Vec<f32> = Vec::with_capacity(last - first + 1);
    for i in first..=last {
        let size = if sized {
            tracks.get(i).map(|t| t.base_size)
        } else {
            fixed.and_then(|f| f.get(i).copied().flatten())
        };
        match size {
            Some(size) => base.push(size),
            None => {
                return super::subgrid_flow::inherit(
                    SubgridAxisTracks {
                        count,
                        sizes: None,
                        gap,
                        names,
                    },
                    parent_reversed != child_reversed,
                );
            }
        }
    }
    let mut pos: Vec<f32> = Vec::with_capacity(base.len());
    if sized && use_offsets {
        let origin = tracks.get(first).map(|t| t.offset).unwrap_or(0.0);
        for i in first..=last {
            let offset = tracks.get(i).map(|t| t.offset).unwrap_or(origin);
            pos.push(if parent_reversed {
                origin - offset - base[i - first]
            } else {
                offset - origin
            });
        }
    } else {
        let mut acc = 0.0;
        for size in &base {
            pos.push(acc);
            acc += size;
        }
    }
    let mut sizes = Vec::with_capacity(count as usize);
    for k in 0..count as usize {
        let t = 1 + 2 * k;
        let start = if k == 0 {
            pos[t] + lead
        } else {
            (pos[t - 2] + base[t - 2] + pos[t]) / 2.0 + gap / 2.0
        };
        let end = if k + 1 == count as usize {
            pos[t] + base[t] - trail
        } else {
            (pos[t] + base[t] + pos[t + 2]) / 2.0 - gap / 2.0
        };
        sizes.push(end - start);
    }
    super::subgrid_flow::inherit(
        SubgridAxisTracks {
            count,
            sizes: Some(sizes),
            gap,
            names,
        },
        parent_reversed != child_reversed,
    )
}
