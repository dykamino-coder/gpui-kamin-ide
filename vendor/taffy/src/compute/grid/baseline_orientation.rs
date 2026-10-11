//! Project first and last x-baseline fallback edges from the item's writing mode.
use crate::{AlignItems, AlignItemsKeyword, Direction};

pub(super) fn projected_x_alignment(
    alignment: AlignItems,
    first_from_end: bool,
    direction: Direction,
) -> AlignItems {
    let from_end = match alignment.keyword {
        AlignItemsKeyword::Baseline => first_from_end,
        AlignItemsKeyword::LastBaseline => !first_from_end,
        _ => return alignment,
    };
    AlignItems {
        keyword: if from_end != direction.is_rtl() {
            AlignItemsKeyword::End
        } else {
            AlignItemsKeyword::Start
        },
        ..alignment
    }
}
