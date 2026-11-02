//! Правила переноса по кускам абзаца с поиском куска по байту за O(log n).
//!
//! Каждая точка разрыва спрашивает правило своего байта (`wrap_at`) и общий
//! предок двух знаков (`nowrap_between`). Линейный поиск по кускам делал
//! фильтрацию точек квадратичной: абзац с тысячами `<span>` тратил на неё
//! треть раскладки. Куски строит `inline::wrap_spans` в порядке текста, без
//! пересечений; это проверяется один раз при сборке списка, и тогда кусок
//! ищется двоичным поиском. Произвольный список (пересечения, другой порядок)
//! сохраняет прежний смысл «первый содержащий кусок» линейным поиском.

use crate::text::paragraph::Wrap;
use std::ops::Range;

/// Отрезки байт со своим `Wrap`; `ordered` выводится из списка при сборке.
#[derive(Clone, Debug, Default, Hash)]
pub(crate) struct WrapSpans {
    list: Vec<(Range<usize>, Wrap)>,
    ordered: bool,
}

impl From<Vec<(Range<usize>, Wrap)>> for WrapSpans {
    fn from(list: Vec<(Range<usize>, Wrap)>) -> Self {
        // Упорядоченные непересекающиеся отрезки: концы не убывают, и первый
        // отрезок с концом правее байта — единственный, кто может его содержать.
        let ordered = list.iter().all(|(r, _)| r.start <= r.end)
            && list.windows(2).all(|w| w[0].0.end <= w[1].0.start);
        Self { list, ordered }
    }
}

impl std::ops::Deref for WrapSpans {
    type Target = [(Range<usize>, Wrap)];
    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl<'a> IntoIterator for &'a WrapSpans {
    type Item = &'a (Range<usize>, Wrap);
    type IntoIter = std::slice::Iter<'a, (Range<usize>, Wrap)>;
    fn into_iter(self) -> Self::IntoIter {
        self.list.iter()
    }
}

impl WrapSpans {
    /// Индекс первого куска, содержащего байт `at`.
    pub(crate) fn index_of(&self, at: usize) -> Option<usize> {
        if !self.ordered {
            return self.list.iter().position(|(r, _)| r.contains(&at));
        }
        let i = self.list.partition_point(|(r, _)| r.end <= at);
        self.list
            .get(i)
            .is_some_and(|(r, _)| r.contains(&at))
            .then_some(i)
    }

    /// Куски, пересекающие полуинтервал `start..end`, в порядке списка.
    pub(crate) fn overlapping(
        &self,
        start: usize,
        end: usize,
    ) -> impl Iterator<Item = &(Range<usize>, Wrap)> {
        let from = if self.ordered {
            self.list.partition_point(|(r, _)| r.end <= start)
        } else {
            0
        };
        let ordered = self.ordered;
        self.list[from..]
            .iter()
            .take_while(move |(r, _)| !ordered || r.start < end)
            .filter(move |(r, _)| r.start < end && r.end > start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(nowrap: bool) -> Wrap {
        Wrap {
            nowrap,
            ..Default::default()
        }
    }

    fn linear(list: &[(Range<usize>, Wrap)], at: usize) -> Option<usize> {
        list.iter().position(|(r, _)| r.contains(&at))
    }

    #[test]
    fn ordered_lookup_matches_first_containing_span() {
        let list = vec![
            (2..4, w(true)),
            (4..4, w(false)),
            (4..7, w(false)),
            (9..10, w(true)),
        ];
        let spans = WrapSpans::from(list.clone());
        assert!(spans.ordered);
        for at in 0..12 {
            assert_eq!(spans.index_of(at), linear(&list, at), "byte {at}");
        }
        let hit: Vec<_> = spans.overlapping(3, 9).map(|(r, _)| r.clone()).collect();
        assert_eq!(hit, vec![2..4, 4..7]);
    }

    #[test]
    fn unordered_or_overlapping_spans_keep_first_match_semantics() {
        let list = vec![(5..9, w(true)), (0..6, w(false)), (1..3, w(true))];
        let spans = WrapSpans::from(list.clone());
        assert!(!spans.ordered);
        for at in 0..10 {
            assert_eq!(spans.index_of(at), linear(&list, at), "byte {at}");
        }
        let hit: Vec<_> = spans.overlapping(2, 4).map(|(r, _)| r.clone()).collect();
        assert_eq!(hit, vec![0..6, 1..3]);
    }

    #[test]
    fn thousands_of_spans_resolve_every_byte() {
        let list: Vec<_> = (0..3000)
            .map(|i| (i * 4..i * 4 + 3, w(i % 2 == 0)))
            .collect();
        let spans = WrapSpans::from(list.clone());
        assert!(spans.ordered);
        for at in (0..12_004).step_by(7) {
            assert_eq!(spans.index_of(at), linear(&list, at));
        }
    }
}
