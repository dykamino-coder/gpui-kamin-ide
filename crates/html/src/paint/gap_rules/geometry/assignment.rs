//! Logical assignment of rule lists, independent of physical gap coordinates.

use crate::style::computed::GapList;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RuleSlot {
    pub index: usize,
    pub count: usize,
}

impl RuleSlot {
    pub fn new(index: usize, count: usize, reverse: bool) -> Self {
        Self {
            index: if reverse { count - 1 - index } else { index },
            count,
        }
    }

    pub fn value<T: Copy>(self, list: &GapList<T>) -> Option<T> {
        list.at(self.index, self.count)
    }
}

/// Physical groups retain their geometry order. Flex consumes one list in
/// logical line order; lanes restart the list, including auto repeat, per lane.
pub(super) fn assign(
    counts: &[usize],
    reverse_groups: bool,
    reverse_gaps: bool,
    restart: bool,
) -> Vec<Vec<RuleSlot>> {
    let total: usize = counts.iter().sum();
    let mut preceding = 0;
    counts
        .iter()
        .map(|&n| {
            let base = if restart {
                0
            } else if reverse_groups {
                total - preceding - n
            } else {
                preceding
            };
            preceding += n;
            (0..n)
                .map(|j| {
                    let slot = RuleSlot::new(j, n, reverse_gaps);
                    RuleSlot {
                        index: base + slot.index,
                        count: if restart { n } else { total },
                    }
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;
    use crate::style::values::value::Len;

    fn values(
        list: &GapList<u8>,
        counts: &[usize],
        groups: bool,
        gaps: bool,
        restart: bool,
    ) -> Vec<Vec<u8>> {
        assign(counts, groups, gaps, restart)
            .into_iter()
            .map(|g| g.into_iter().map(|s| s.value(list).unwrap()).collect())
            .collect()
    }

    #[test]
    fn flex_sequence_follows_both_logical_axes() {
        let list = GapList {
            lead: vec![1, 2, 3, 4, 5],
            auto: vec![],
            tail: vec![],
        };
        assert_eq!(
            values(&list, &[2, 0, 3], false, false, false),
            vec![vec![1, 2], vec![], vec![3, 4, 5]]
        );
        assert_eq!(
            values(&list, &[2, 0, 3], false, true, false),
            vec![vec![2, 1], vec![], vec![5, 4, 3]]
        );
        assert_eq!(
            values(&list, &[2, 0, 3], true, false, false),
            vec![vec![4, 5], vec![], vec![1, 2, 3]]
        );
        assert_eq!(
            values(&list, &[2, 0, 3], true, true, false),
            vec![vec![5, 4], vec![], vec![3, 2, 1]]
        );
    }

    #[test]
    fn lanes_auto_repeat_uses_each_lane_count_before_reversal() {
        let list = GapList {
            lead: vec![1],
            auto: vec![2, 3],
            tail: vec![4],
        };
        assert_eq!(
            values(&list, &[4, 2, 5], false, false, true),
            vec![vec![1, 2, 3, 4], vec![1, 4], vec![1, 2, 3, 2, 4]]
        );
        assert_eq!(
            values(&list, &[4, 2], true, true, true),
            vec![vec![4, 3, 2, 1], vec![4, 1]]
        );
        assert_eq!(
            values(&list, &[0, 1], false, true, true),
            vec![vec![], vec![1]]
        );
    }

    #[test]
    fn integer_repeat_cycles_after_expansion_and_reverses_used_values() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut style = Computed::default();
        style.apply_one("column-rule-width", "repeat(2, 1px, 2px), 3px", vcx);
        let list = style.column_rule_widths.as_ref().unwrap();
        let slots = assign(&[7], false, true, false);
        let got: Vec<_> = slots[0].iter().map(|s| s.value(list).unwrap()).collect();
        assert_eq!(got, [2., 1., 3., 2., 1., 2., 1.].map(Len::Px));
    }

    #[test]
    fn none_style_consumes_a_slot() {
        let styles = GapList {
            lead: vec![true, false, true],
            auto: vec![],
            tail: vec![],
        };
        let slots = assign(&[3], false, true, false);
        assert_eq!(
            slots[0]
                .iter()
                .map(|s| s.value(&styles))
                .collect::<Vec<_>>(),
            [Some(true), Some(false), Some(true)]
        );
        assert!(assign(&[], true, true, false).is_empty());
    }
}
