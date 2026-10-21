//! CSS Tables 3 visibility collapse: size the full grid, then remove collapsed tracks.
use super::types::GridTrack;

/// KaminIDE patch: Whether a table row is collapsed.
fn removed(index: usize, rows: &[u16]) -> bool {
    if index % 2 == 1 {
        rows.contains(&((index / 2) as u16))
    } else if index == 0 {
        false
    } else {
        let next = index / 2;
        rows.contains(&(next as u16)) || (0..next).all(|r| rows.contains(&(r as u16)))
    }
}

/// KaminIDE patch: Sum track sizes after excluding collapsed table rows.
pub(super) fn visible_sum(
    tracks: &[GridTrack],
    collapse: Option<&(bool, Vec<u16>)>,
    horizontal: bool,
) -> f32 {
    tracks
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            !collapse.is_some_and(|(axis, rows)| *axis == horizontal && removed(*i, rows))
        })
        .map(|(_, track)| f64::from(track.base_size))
        .sum::<f64>() as f32
}

/// KaminIDE patch: Apply collapsed table-row adjustments to grid geometry.
pub(super) fn apply(
    tracks: &mut [GridTrack],
    collapse: Option<&(bool, Vec<u16>)>,
    horizontal: bool,
) {
    let Some((axis, rows)) = collapse else { return };
    if *axis != horizontal {
        return;
    }
    for (i, track) in tracks.iter_mut().enumerate() {
        if removed(i, rows) {
            track.collapse();
            track.base_size = 0.0;
            track.growth_limit = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::removed;

    #[test]
    fn collapsed_rows_leave_one_gutter_between_visible_neighbors() {
        // Three rows alternating with gutters, including zero edge gutters.
        assert_eq!(
            (0..7).filter(|i| removed(*i, &[1])).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert_eq!(
            (0..7).filter(|i| removed(*i, &[0])).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            (0..7).filter(|i| removed(*i, &[0, 1])).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
    }
}
