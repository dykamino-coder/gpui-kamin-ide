//! Коробки дерева по номеру исходного элемента без обхода: родитель и номер
//! среди детей у каждой коробки, первая коробка у каждого номера. Дерево
//! коробок — значение без обратных ссылок (копия пути при записи), поэтому
//! индекс строится одним обходом при первой правке на месте и служит, пока
//! структура дерева та же: правка на месте (`Plan`) меняет только атрибуты,
//! стили и тексты коробок, а сборка даёт документ с новым индексом.

use super::patch::{Path, elements};
use crate::dom::Node;
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
    /// Первая коробка по `node_id`.
    first: Vec<u32>,
    /// Остальные коробки номеров с копиями коробки.
    more: HashMap<u64, Vec<u32>>,
}

impl Boxes {
    /// Индекс дерева `nodes`; `None`, если номеров коробок больше `u32`.
    pub(super) fn of(nodes: &[Node]) -> Option<Self> {
        let mut boxes = Self::default();
        let mut chain: Vec<u32> = Vec::new();
        let mut fits = true;
        elements(nodes, |e, path| {
            let at = boxes.parent.len();
            let (Ok(at), Ok(index), Ok(id)) = (
                u32::try_from(at),
                u32::try_from(path[path.len() - 1]),
                usize::try_from(e.node_id),
            ) else {
                fits = false;
                return false;
            };
            chain.truncate(path.len() - 1);
            boxes.parent.push(chain.last().copied().unwrap_or(NONE));
            boxes.index.push(index);
            chain.push(at);
            if e.node_id != 0 {
                if boxes.first.len() <= id {
                    boxes.first.resize(id + 1, NONE);
                }
                match boxes.first[id] {
                    NONE => boxes.first[id] = at,
                    _ => boxes.more.entry(e.node_id).or_default().push(at),
                }
            }
            true
        });
        fits.then_some(boxes)
    }

    /// Пути ко всем коробкам номера `id` за O(глубины) на коробку.
    pub(super) fn paths(&self, id: u64) -> impl Iterator<Item = Path> + '_ {
        let first = usize::try_from(id)
            .ok()
            .and_then(|i| self.first.get(i).copied())
            .filter(|&at| at != NONE && id != 0);
        let more = self.more.get(&id).into_iter().flatten().copied();
        first.into_iter().chain(more).map(|at| self.path(at))
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
                assert_eq!(
                    super::super::patch::element_at(doc.nodes(), path).map(|e| e.node_id),
                    Some(*id)
                );
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
}
