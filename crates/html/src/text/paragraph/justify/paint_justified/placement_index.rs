//! Логические интервалы непересекающихся визуальных кусков строки:
//! один индекс для размещения слов и выбора полосы их строчных коробок.

use gpui::Pixels;

pub(crate) struct PlacementIndex {
    pieces: Vec<(usize, usize, usize)>,
}

impl PlacementIndex {
    pub(super) fn new(placed: &[(usize, usize, bool, Pixels)]) -> Self {
        let mut pieces: Vec<_> = placed
            .iter()
            .enumerate()
            .map(|(i, p)| (p.0, p.1, i))
            .collect();
        pieces.sort_unstable_by_key(|p| p.0);
        Self { pieces }
    }

    pub(super) fn at(&self, at: usize) -> Option<usize> {
        let i = self.pieces.partition_point(|p| p.0 <= at).checked_sub(1)?;
        let &(_, end, original) = &self.pieces[i];
        (at < end).then_some(original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn logical_lookup_preserves_visual_piece_identity() {
        let placed = [
            (6, 9, true, px(0.)),
            (0, 3, false, px(4.)),
            (3, 6, false, px(8.)),
        ];
        let index = PlacementIndex::new(&placed);
        for at in 0..=12 {
            assert_eq!(
                index.at(at),
                placed.iter().position(|p| p.0 <= at && at < p.1)
            );
        }
        assert_eq!(PlacementIndex::new(&[]).at(0), None);
    }
}
