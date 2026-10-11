//! CSS Grid 3 stacking baselines come from the first/last usable item in each track.
//! Placement order is independent of relative positioning and final alignment offsets.

use crate::Baselines;

pub(super) fn compute(
    placed: &[super::Placed],
    first_in_track: &[Option<usize>],
    rows: bool,
    stack_start_inset: f32,
) -> Baselines {
    if rows {
        return Baselines {
            first: first_in_track.iter().find_map(|&index| index).map(|index| {
                let item = &placed[index];
                item.grid_pos + item.baseline.unwrap_or(item.size.height)
            }),
            last: None,
        };
    }
    let candidates: crate::util::sys::Vec<_> = placed
        .iter()
        .map(|item| {
            let offset = stack_start_inset + item.stack_pos + item.margin.top + item.stack_relative;
            Candidate {
                start: item.start,
                end: item.end,
                placement: item.placement_stack_pos,
                first: item.baseline.map(|value| offset + value),
                last: item.last_baseline.map(|value| offset + value),
            }
        })
        .collect();
    columns(&candidates, first_in_track.len())
}

struct Candidate {
    start: usize,
    end: usize,
    placement: f32,
    first: Option<f32>,
    last: Option<f32>,
}

fn columns(items: &[Candidate], tracks: usize) -> Baselines {
    let mut output = Baselines::NONE;
    for track in 0..tracks {
        let mut first: Option<&Candidate> = None;
        let mut last: Option<&Candidate> = None;
        for item in items {
            if item.start > track || item.end <= track || item.first.is_none() {
                continue;
            }
            // Dense placement may fill an earlier opening after this track was occupied.
            if first.map_or(true, |old| item.placement < old.placement) {
                first = Some(item);
            }
            if last.map_or(true, |old| item.placement >= old.placement) {
                last = Some(item);
            }
        }
        if let Some(value) = first.and_then(|item| item.first) {
            output.first = Some(output.first.map_or(value, |old| old.min(value)));
        }
        if let Some(value) = last.and_then(|item| item.last.or(item.first)) {
            output.last = Some(output.last.map_or(value, |old| old.max(value)));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(track: usize, placement: f32, first: f32, last: f32) -> Candidate {
        Candidate {
            start: track,
            end: track + 1,
            placement,
            first: Some(first),
            last: Some(last),
        }
    }

    #[test]
    fn both_container_edges_include_every_occupied_track() {
        let items = [
            item(0, 0.0, 30.0, 40.0),
            item(1, 0.0, 18.0, 24.0),
            item(0, 50.0, 65.0, 90.0),
            item(1, 35.0, 52.0, 70.0),
        ];
        assert_eq!(
            columns(&items, 2),
            Baselines {
                first: Some(18.0),
                last: Some(90.0)
            }
        );
    }

    #[test]
    fn dense_backfill_replaces_the_first_item_but_preserves_the_last() {
        let items = [item(0, 50.0, 64.0, 70.0), item(0, 0.0, 15.0, 22.0)];
        assert_eq!(
            columns(&items, 1),
            Baselines {
                first: Some(15.0),
                last: Some(70.0)
            }
        );
    }

    #[test]
    fn relative_offsets_do_not_select_a_different_child() {
        let items = [item(0, 0.0, 215.0, 220.0), item(0, 50.0, -35.0, -20.0)];
        assert_eq!(
            columns(&items, 1),
            Baselines {
                first: Some(215.0),
                last: Some(-20.0)
            }
        );
    }

    #[test]
    fn empty_boxes_do_not_create_a_synthetic_content_baseline() {
        let items = [
            Candidate {
                start: 0,
                end: 1,
                placement: 0.0,
                first: None,
                last: None,
            },
            item(0, 30.0, 45.0, 48.0),
        ];
        assert_eq!(
            columns(&items, 1),
            Baselines {
                first: Some(45.0),
                last: Some(48.0)
            }
        );
        assert_eq!(columns(&items[..1], 1), Baselines::NONE);
    }

    #[test]
    fn spanning_child_participates_in_every_covered_track() {
        let items = [
            Candidate {
                start: 0,
                end: 2,
                placement: 0.0,
                first: Some(15.0),
                last: Some(19.0),
            },
            item(1, 0.0, 10.0, 25.0),
            item(1, 40.0, 52.0, 58.0),
        ];
        assert_eq!(
            columns(&items, 2),
            Baselines {
                first: Some(15.0),
                last: Some(58.0)
            }
        );
    }
}
