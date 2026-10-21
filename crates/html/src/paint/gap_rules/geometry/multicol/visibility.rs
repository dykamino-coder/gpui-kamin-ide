//! Видимость `row-rule` многоколонника по колонке (css-gaps-1 §visibility-items): `around` —
//! колонка занята хотя бы в одном из соседних рядов, `between` — в обоих, `normal` в
//! многоколоннике ведёт себя как `all`.

pub(super) fn row_visible(before: usize, after: usize, column: usize, visibility: u8) -> bool {
    match visibility {
        2 => column < before || column < after,
        3 => column < before && column < after,
        _ => true, // CSS Gaps: normal row visibility in multicol behaves as all.
    }
}

#[cfg(test)]
mod tests {
    use super::row_visible;

    #[test]
    fn incomplete_last_row_distinguishes_between_around_and_normal() {
        for (visibility, expected) in [
            (0, [true; 3]),
            (1, [true; 3]),
            (2, [true; 3]),
            (3, [true, false, false]),
        ] {
            assert_eq!(
                (0..3)
                    .map(|c| row_visible(3, 1, c, visibility))
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                (0..3)
                    .map(|c| row_visible(1, 3, c, visibility))
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert!(!row_visible(0, 0, 0, 2));
        assert!(!row_visible(0, 3, 0, 3));
    }
}
