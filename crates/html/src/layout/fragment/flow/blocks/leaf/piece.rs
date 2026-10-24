//! Геометрия leaf-фрагмента: slice продолжает края отдельно от normal content.

use super::{Fragment, Leaf};

pub(super) struct Piece {
    pub(super) fragment: Fragment,
    pub(super) take: f32,
    pub(super) complete: bool,
    pub(super) sliced: Option<(f32, f32)>,
}

impl Piece {
    pub(super) fn new(leaf: Leaf, from: f32, first: bool, sliced: (f32, f32), room: f32) -> Self {
        let rest = leaf.content_block - from;
        if leaf.edges.clone {
            let fits = rest + leaf.edges.sum() <= room || rest == 0.0;
            let edges = if fits {
                leaf.edges
            } else {
                leaf.edges.fit_clone(room, rest)
            };
            let take = if fits {
                rest
            } else {
                rest.min((room - edges.sum()).max(0.0))
            };
            return Self {
                fragment: Fragment {
                    from,
                    block_size: edges.sum() + take,
                },
                take,
                complete: fits || from + take == leaf.content_block,
                sliced: None,
            };
        }
        let (start_from, end_from) = (leaf.edges.start * sliced.0, leaf.edges.end * sliced.1);
        let start = (leaf.edges.start - start_from).min(room);
        let take = if start_from + start == leaf.edges.start {
            rest.min((room - start).max(0.0))
        } else {
            0.0
        };
        let end = if from + take == leaf.content_block && start_from + start == leaf.edges.start {
            (leaf.edges.end - end_from).min((room - start - take).max(0.0))
        } else {
            0.0
        };
        let fraction = |consumed, total| if total == 0.0 { 1.0 } else { consumed / total };
        let sliced = (
            fraction(start_from + start, leaf.edges.start),
            fraction(end_from + end, leaf.edges.end),
        );
        Self {
            fragment: Fragment {
                from: if first {
                    0.0
                } else {
                    start_from + from + end_from
                },
                block_size: start + take + end,
            },
            take,
            complete: sliced == (1.0, 1.0) && from + take == leaf.content_block,
            sliced: Some(sliced),
        }
    }
}
