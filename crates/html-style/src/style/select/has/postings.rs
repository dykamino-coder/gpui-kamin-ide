//! Заимствованные posting lists или невыделяющий память диапазон всего документа.

pub(in crate::style::select) struct Candidates<'a> {
    list: Option<&'a [usize]>,
    count: usize,
}

impl<'a> Candidates<'a> {
    pub fn new(list: Option<&'a [usize]>, count: usize) -> Self {
        Self { list, count }
    }

    pub fn len(&self) -> usize {
        self.list.map_or(self.count, <[usize]>::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.window(0, self.count)
    }

    /// Оба представления ограничиваются preorder-интервалом без копирования.
    pub fn window(&self, start: usize, end: usize) -> impl Iterator<Item = usize> + '_ {
        let (begin, end) = self.list.map_or_else(
            || (start.min(self.count), end.min(self.count)),
            |list| {
                (
                    list.partition_point(|&i| i < start),
                    list.partition_point(|&i| i < end),
                )
            },
        );
        (begin..end).map(|i| self.list.map_or(i, |list| list[i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posting_and_universal_windows_keep_preorder_and_exclude_the_end() {
        let all = Candidates::new(None, 8);
        let posting = Candidates::new(Some(&[0, 2, 3, 7]), 8);
        assert_eq!(all.iter().collect::<Vec<_>>(), (0..8).collect::<Vec<_>>());
        assert_eq!(posting.iter().collect::<Vec<_>>(), [0, 2, 3, 7]);
        assert_eq!(all.window(2, 7).collect::<Vec<_>>(), [2, 3, 4, 5, 6]);
        assert_eq!(posting.window(2, 7).collect::<Vec<_>>(), [2, 3]);
        assert_eq!(all.window(9, 12).count(), 0);
        assert_eq!(posting.window(9, 12).count(), 0);
        assert_eq!(posting.window(2, 2).count(), 0);
        assert!(Candidates::new(Some(&[]), 8).is_empty());
        assert!(Candidates::new(None, 0).is_empty());
    }
}
