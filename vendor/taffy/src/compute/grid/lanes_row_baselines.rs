//! Row grid-lanes export grid-axis baselines using CSS Grid's occupied-edge rule.
//! Sharing groups precede available child baselines and border-box synthesis.

use crate::Baselines;

pub(super) fn compute(placed: &[super::super::Placed]) -> Baselines {
    let items: crate::util::sys::Vec<_> = placed
        .iter()
        .map(|item| Candidate {
            start: item.start,
            end: item.end,
            placement: item.placement_stack_pos,
            offset: item.grid_pos,
            height: item.size.height,
            first: item.baseline,
            last: item.last_baseline,
            first_shared: item.grid_baseline,
            last_shared: item.grid_last_baseline,
        })
        .collect();
    baselines(&items)
}

struct Candidate {
    start: usize,
    end: usize,
    placement: f32,
    offset: f32,
    height: f32,
    first: Option<f32>,
    last: Option<f32>,
    first_shared: bool,
    last_shared: bool,
}

impl Candidate {
    fn measured(&self, last: bool) -> Option<f32> {
        if last {
            self.last.or(self.first)
        } else {
            self.first.or(self.last)
        }
    }

    fn coordinate(&self, last: bool) -> f32 {
        self.offset + self.measured(last).unwrap_or(self.height)
    }

    fn shared(&self, last: bool) -> bool {
        if last {
            self.last_shared
        } else {
            self.first_shared
        }
    }
}

fn select(
    items: &[Candidate],
    accepts: impl Fn(&Candidate) -> bool,
    last: bool,
) -> Option<&Candidate> {
    let mut selected: Option<&Candidate> = None;
    for item in items.iter().filter(|item| accepts(item)) {
        if selected.map_or(true, |old| {
            if last {
                item.start > old.start
                    || (item.start == old.start && item.placement >= old.placement)
            } else {
                item.start < old.start
                    || (item.start == old.start && item.placement < old.placement)
            }
        }) {
            selected = Some(item);
        }
    }
    selected
}

fn baseline(items: &[Candidate], last: bool) -> Option<f32> {
    let edge = if last {
        items.iter().map(|item| item.end).max()?
    } else {
        items.iter().map(|item| item.start).min()?
    };
    let at_edge = |item: &Candidate| {
        if last {
            item.end == edge
        } else {
            item.start == edge
        }
    };
    if let Some(item) = select(items, |item| at_edge(item) && item.shared(last), last) {
        return Some(item.coordinate(last));
    }
    let opposite_edge = |item: &Candidate| {
        if last {
            item.start + 1 == edge
        } else {
            item.end == edge + 1
        }
    };
    if let Some(item) = select(
        items,
        |item| opposite_edge(item) && item.shared(!last),
        last,
    ) {
        return Some(item.coordinate(!last));
    }
    if let Some(item) = select(
        items,
        |item| at_edge(item) && item.measured(last).is_some(),
        last,
    ) {
        return Some(item.coordinate(last));
    }
    select(items, |_| true, last).map(|item| item.offset + item.height)
}

fn baselines(items: &[Candidate]) -> Baselines {
    Baselines {
        first: baseline(items, false),
        last: baseline(items, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(start: usize, end: usize, placement: f32) -> Candidate {
        Candidate {
            start,
            end,
            placement,
            offset: 10.0 * start as f32,
            height: 40.0,
            first: Some(12.0),
            last: Some(30.0),
            first_shared: false,
            last_shared: false,
        }
    }

    #[test]
    fn spanning_subgrid_exports_both_content_edges() {
        assert_eq!(
            baselines(&[item(0, 2, 0.0)]),
            Baselines {
                first: Some(12.0),
                last: Some(30.0)
            }
        );
    }

    #[test]
    fn last_occupied_row_uses_grid_order_before_relative_offsets() {
        let mut later = item(1, 2, 20.0);
        later.offset = -100.0;
        assert_eq!(
            baselines(&[later, item(1, 2, 0.0), item(0, 1, 0.0)]),
            Baselines {
                first: Some(12.0),
                last: Some(-70.0)
            }
        );
    }

    #[test]
    fn sharing_group_precedes_other_available_baselines() {
        let mut shared = item(0, 1, 0.0);
        shared.first_shared = true;
        assert_eq!(baselines(&[shared, item(0, 1, 20.0)]).last, Some(12.0));
    }

    #[test]
    fn empty_box_synthesis_uses_last_item_in_grid_order() {
        let mut empty = item(1, 2, 0.0);
        empty.first = None;
        empty.last = None;
        assert_eq!(
            baselines(&[item(0, 1, 0.0), empty]),
            Baselines {
                first: Some(12.0),
                last: Some(50.0)
            }
        );
        assert_eq!(baselines(&[]), Baselines::NONE);
    }
}
