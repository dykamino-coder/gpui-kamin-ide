//! Завершение пакета продвижений атомов: новые записи получают тот же
//! приоритет, что при последовательных вставках в начало, без сдвига всего
//! старого массива для каждого атома. Память принадлежит абзацу.

use gpui::Pixels;
use std::ops::Range;

pub(super) fn prepend_appended(spans: &mut [(Range<usize>, Pixels)], old_len: usize) {
    let added = spans.len() - old_len;
    spans[old_len..].reverse();
    spans.rotate_right(added);
    crate::work::add!(
        ParagraphAdvanceMoves,
        2 * (added / 2)
            + if old_len > 0 && added > 0 {
                spans.len()
            } else {
                0
            }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn new_atom_advances_keep_reverse_priority_over_old_overlapping_spans() {
        let mut spans = vec![(0..9, px(1.)), (0..3, px(2.))];
        let old_len = spans.len();
        spans.extend([(0..3, px(7.)), (3..6, px(8.)), (0..3, px(9.))]);
        prepend_appended(&mut spans, old_len);
        assert_eq!(
            spans,
            vec![
                (0..3, px(9.)),
                (3..6, px(8.)),
                (0..3, px(7.)),
                (0..9, px(1.)),
                (0..3, px(2.)),
            ]
        );
        assert_eq!(
            spans.iter().find(|(r, _)| r.contains(&0)).unwrap().1,
            px(9.)
        );
    }

    #[test]
    fn empty_prefix_and_empty_append_need_no_special_allocation() {
        let mut spans = vec![(3..6, px(0.1)), (0..3, px(-0.))];
        let allocation = spans.as_ptr();
        prepend_appended(&mut spans, 0);
        assert_eq!(spans[0].0, 0..3);
        assert_eq!(f32::from(spans[0].1).to_bits(), (-0.0f32).to_bits());
        let old_len = spans.len();
        prepend_appended(&mut spans, old_len);
        assert_eq!(spans.as_ptr(), allocation);
        assert_eq!(spans[0].0, 0..3);
        prepend_appended(&mut [], 0);
    }
}
