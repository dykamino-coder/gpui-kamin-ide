//! Индекс диапазонов абзаца: пересечения без обхода всех стилей; порядок
//! исходного списка сохраняет выбор первого стиля и порядок рисования.

use std::ops::Range;

/// Индекс живёт в каждом подготовленном абзаце кадра: пустой набор
/// диапазонов (обычный случай) не занимает ничего, кроме указателя.
#[derive(Default)]
pub(in crate::text::paragraph) struct Intervals(Option<Box<Tree>>);

#[derive(Default)]
struct Tree {
    nodes: Vec<Node>,
    root: Option<usize>,
    first: Vec<(usize, Option<usize>)>,
}

struct Node {
    range: Range<usize>,
    source: usize,
    max_end: usize,
    left: Option<usize>,
    right: Option<usize>,
}

impl Intervals {
    pub(in crate::text::paragraph) fn new(ranges: impl Iterator<Item = Range<usize>>) -> Self {
        let mut entries: Vec<_> = ranges.enumerate().collect();
        if entries.is_empty() {
            return Self(None);
        }
        entries.sort_unstable_by_key(|(i, r)| (r.start, *i));
        let mut out = Tree {
            nodes: Vec::with_capacity(entries.len()),
            ..Tree::default()
        };
        out.root = out.build(&entries);
        let mut events: Vec<_> = entries
            .iter()
            .filter(|(_, r)| r.start < r.end)
            .flat_map(|(i, r)| [(r.start, true, *i), (r.end, false, *i)])
            .collect();
        events.sort_unstable();
        let mut active = std::collections::BTreeSet::new();
        for (at, add, i) in events {
            if add {
                active.insert(i);
            } else {
                active.remove(&i);
            }
            let value = active.first().copied();
            if let Some(last) = out.first.last_mut().filter(|last| last.0 == at) {
                last.1 = value;
            } else {
                out.first.push((at, value));
            }
        }
        out.first.shrink_to_fit();
        Self(Some(Box::new(out)))
    }

    pub(in crate::text::paragraph) fn overlapping(&self, range: &Range<usize>) -> Vec<usize> {
        let mut out = Vec::new();
        if let Some(tree) = &self.0 {
            // Прежняя проверка строгих краёв учитывает пустой диапазон,
            // лежащий внутри другого: это влияет на надбавки пустых строк.
            tree.visit(tree.root, range, &mut out);
            out.sort_unstable();
        }
        out
    }

    pub(in crate::text::paragraph) fn first(&self, at: usize) -> Option<usize> {
        let first = &self.0.as_ref()?.first;
        let i = first.partition_point(|(start, _)| *start <= at);
        i.checked_sub(1).and_then(|i| first[i].1)
    }
}

impl Tree {
    fn build(&mut self, entries: &[(usize, Range<usize>)]) -> Option<usize> {
        if entries.is_empty() {
            return None;
        }
        let mid = entries.len() / 2;
        let left = self.build(&entries[..mid]);
        let right = self.build(&entries[mid + 1..]);
        let (source, range) = &entries[mid];
        let max_end = left
            .into_iter()
            .chain(right)
            .fold(range.end, |end, i| end.max(self.nodes[i].max_end));
        let i = self.nodes.len();
        self.nodes.push(Node {
            range: range.clone(),
            source: *source,
            max_end,
            left,
            right,
        });
        Some(i)
    }

    fn visit(&self, node: Option<usize>, range: &Range<usize>, out: &mut Vec<usize>) {
        let Some(i) = node else {
            return;
        };
        let node = &self.nodes[i];
        if node.max_end <= range.start {
            return;
        }
        self.visit(node.left, range, out);
        if node.range.start >= range.end {
            return;
        }
        if node.range.end > range.start {
            out.push(node.source);
        }
        self.visit(node.right, range, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsorted_nested_ranges_retain_source_order() {
        let ranges = [4..8, 0..20, 4..6, 9..9, 2..4];
        let index = Intervals::new(ranges.iter().cloned());
        for start in 0..22 {
            for end in start..22 {
                let expected: Vec<_> = ranges
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r.start < end && start < r.end)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(index.overlapping(&(start..end)), expected);
            }
            assert_eq!(
                index.first(start),
                ranges.iter().position(|r| r.contains(&start))
            );
        }
    }

    #[test]
    fn empty_index_owns_no_storage_and_answers_nothing() {
        let index = Intervals::new(std::iter::empty());
        assert!(index.0.is_none());
        assert!(index.overlapping(&(0..10)).is_empty());
        assert_eq!(index.first(0), None);
        assert_eq!(size_of::<Intervals>(), size_of::<usize>());
    }
}
