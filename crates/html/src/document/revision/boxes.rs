//! Коробки дерева по номеру исходного элемента без обхода: родитель и номер
//! среди детей у каждой коробки, первая коробка у каждого номера. Дерево
//! коробок — значение без обратных ссылок (копия пути при записи), поэтому
//! индекс строится одним обходом при первой правке на месте и служит, пока
//! структура дерева та же: правка на месте (`Plan`) меняет только атрибуты,
//! стили и тексты коробок, а сборка даёт документ с новым индексом.

use super::patch::{Path, elements};
use crate::dom::{Element, Node};
use std::collections::HashMap;

/// Нет коробки (родителя у коробки верхнего уровня, коробки у номера).
const NONE: u32 = u32::MAX;

/// Индекс коробок одного дерева (`Kept`).
#[derive(Default)]
pub(in crate::document) struct Boxes {
    /// Родитель каждой коробки в прямом порядке обхода.
    parent: Vec<u32>,
    /// Номер коробки в списке детей родителя (с текстами).
    index: Vec<u32>,
    /// Первая коробка по `node_id - base`. Ключи узлов документа идут подряд
    /// из его счётчика: плотный вектор от наименьшего ключа дерева, а при
    /// разреженных ключах (долгая череда правок) — только `more`.
    first: Vec<u32>,
    base: u64,
    /// Остальные коробки номеров с копиями коробки.
    more: HashMap<u64, Vec<u32>>,
}

impl Boxes {
    /// Индекс дерева `nodes`; `None`, если номеров коробок больше `u32`.
    pub(super) fn of(nodes: &[Node]) -> Option<Self> {
        let mut boxes = Self::default();
        let mut chain: Vec<u32> = Vec::new();
        let mut fits = true;
        let mut keyed: Vec<(u64, u32)> = Vec::new();
        elements(nodes, |e, path| {
            let at = boxes.parent.len();
            let (Ok(at), Ok(index)) = (u32::try_from(at), u32::try_from(path[path.len() - 1]))
            else {
                fits = false;
                return false;
            };
            chain.truncate(path.len() - 1);
            boxes.parent.push(chain.last().copied().unwrap_or(NONE));
            boxes.index.push(index);
            chain.push(at);
            if e.node_id != 0 {
                keyed.push((e.node_id, at));
            }
            true
        });
        fits.then(|| boxes.keyed(keyed))
    }

    fn keyed(mut self, keyed: Vec<(u64, u32)>) -> Self {
        let (min, max) = keyed
            .iter()
            .fold((u64::MAX, 0), |(lo, hi), &(id, _)| (lo.min(id), hi.max(id)));
        let span = max.saturating_sub(min).saturating_add(1);
        let dense = !keyed.is_empty() && span <= 4 * keyed.len() as u64 + 64;
        if dense {
            self.base = min;
            self.first = vec![NONE; span as usize];
        }
        for (id, at) in keyed {
            let first = dense.then(|| &mut self.first[(id - min) as usize]);
            match first {
                Some(first) if *first == NONE => *first = at,
                _ => self.more.entry(id).or_default().push(at),
            }
        }
        self
    }

    /// Пути ко всем коробкам номера `id` за O(глубины) на коробку.
    pub(super) fn paths(&self, id: u64) -> impl Iterator<Item = Path> + '_ {
        let first = id
            .checked_sub(self.base)
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| self.first.get(i).copied())
            .filter(|&at| at != NONE && id != 0);
        let more = self.more.get(&id).into_iter().flatten().copied();
        first.into_iter().chain(more).map(|at| self.path(at))
    }

    /// A source with exactly one occurrence addresses its box directly.
    pub(super) fn unique(&self, id: u64) -> bool {
        id != 0
            && match self.more.get(&id) {
                None => true,
                Some(copies) => self.first.is_empty() && copies.len() == 1,
            }
    }

    fn path(&self, mut at: u32) -> Path {
        let mut path = Vec::new();
        while at != NONE {
            path.push(self.index[at as usize] as usize);
            at = self.parent[at as usize];
        }
        path.reverse();
        path
    }
}

/// Коробка по пути `path` (без копирования).
pub(super) fn at<'a>(nodes: &'a [Node], path: &[usize]) -> Option<&'a Element> {
    let (last, parents) = path.split_last()?;
    let mut list = nodes;
    for &i in parents {
        let Some(Node::Element(e)) = list.get(i) else {
            return None;
        };
        list = &e.children;
    }
    match list.get(*last) {
        Some(Node::Element(e)) => Some(e),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Document;

    /// Every box of a tree is found by its number at its own path, and no
    /// other path is returned.
    fn same_as_walk(html: &str) {
        let doc = Document::builder(html, "").build().unwrap();
        let boxes = Boxes::of(doc.nodes()).unwrap();
        let mut walked: HashMap<u64, Vec<Path>> = HashMap::new();
        elements(doc.nodes(), |e, path| {
            if e.node_id != 0 {
                walked.entry(e.node_id).or_default().push(path.to_vec());
            }
            true
        });
        assert!(!walked.is_empty());
        for (id, paths) in &walked {
            let mut found: Vec<Path> = boxes.paths(*id).collect();
            found.sort();
            let mut paths = paths.clone();
            paths.sort();
            assert_eq!(found, paths, "{id}");
            for path in &found {
                assert_eq!(at(doc.nodes(), path).map(|e| e.node_id), Some(*id));
            }
        }
        assert_eq!(boxes.paths(0).count(), 0);
    }

    #[test]
    fn every_box_is_found_by_its_number_at_its_path() {
        same_as_walk(
            "<!doctype html><style>body{background:#fff}</style><body><h1>t</h1>\
             <div>a<p>b <span>c</span> d</p><ul><li>x</li><li>y</li></ul></div>\
             <table><tr><td>1</td></tr></table><span>inline <b>run</b></span></body>",
        );
    }

    #[test]
    fn boxes_under_unwrapped_wrappers_are_found_too() {
        same_as_walk("<!doctype html><p>a</p><div><p>b<i>c</i></p></div>text<p>d</p>");
    }

    /// Node keys keep growing across a document's edits: the index is dense
    /// from the smallest key of the tree and falls back to the map when sparse.
    #[test]
    fn keys_far_from_zero_or_far_apart_do_not_size_the_index() {
        let flat = |ids: &[u64]| {
            Boxes {
                parent: vec![NONE; ids.len()],
                index: (0..ids.len() as u32).collect(),
                ..Default::default()
            }
            .keyed(ids.iter().zip(0..).map(|(&id, at)| (id, at)).collect())
        };
        let paths = |boxes: &Boxes, id| {
            let mut found: Vec<Path> = boxes.paths(id).collect();
            found.sort();
            found
        };
        let dense = flat(&[1_000_000, 1_000_001, 1_000_000]);
        assert_eq!(dense.first.len(), 2);
        assert_eq!(paths(&dense, 1_000_000), vec![vec![0], vec![2]]);
        assert_eq!(paths(&dense, 1_000_001), vec![vec![1]]);
        assert!(paths(&dense, 999_999).is_empty());
        let sparse = flat(&[3, u64::MAX / 2]);
        assert!(sparse.first.is_empty());
        assert_eq!(paths(&sparse, u64::MAX / 2), vec![vec![1]]);
        assert_eq!(paths(&sparse, 3), vec![vec![0]]);
    }
}
