//! Подготовка снимка документа: факты post-order и индексы краски одним обходом.

use super::SubtreeFacts;
use crate::dom::{Element, Node};
use crate::layout::order::{Unkeyed, unkeyed_positioned};
use std::rc::Rc;

pub(crate) struct Preparation {
    pub(crate) unkeyed: Rc<Unkeyed>,
    pub(crate) clearance_possible: bool,
}

type Snapshot = (u64, Rc<Preparation>);

/// Ключ снимка-основы (`continuing`): номер дерева документа не бывает нулевым
/// (`Document::tree`), поэтому основа не совпадает ни с одним деревом и не
/// требует своего поля.
const BASE: u64 = 0;

#[derive(Default)]
pub(crate) struct PreparationCache {
    /// Подготовка последнего дерева или, под ключом `BASE`, подготовка дерева
    /// прошлой ревизии той же формы и стилей (`continuing`).
    snapshot: std::cell::Cell<Option<Snapshot>>,
}

impl PreparationCache {
    /// Кэш ревизии, дерево которой — копия путей к правкам атрибутов и
    /// текстов прошлого (`Document::adopted`): та же форма, те же стили и
    /// номера узлов. Подготовка прошлого дерева — основа первой подготовки:
    /// индексы краски и возможность прояснения те же, а факты нужны только
    /// новым спискам детей на путях к правкам (Servo `traversal.rs:87`: обход
    /// ниже грязного корня, чистые поддеревья не посещаются).
    pub(crate) fn continuing(previous: &Self) -> Self {
        let snapshot = previous.snapshot.take();
        let base = snapshot
            .as_ref()
            .map(|(_, prepared)| (BASE, prepared.clone()));
        previous.snapshot.set(snapshot);
        Self {
            snapshot: std::cell::Cell::new(base),
        }
    }

    /// `tree`: номер дерева документа (`Document::tree`), новый у каждого
    /// дерева; только Document гарантирует неизменность дерева под номером.
    /// Адрес корневого списка ключом не годится: правка текста, давшая новое
    /// дерево, может положить его корень по адресу прошлого. Для raw render
    /// (`None`) кэш не применяется.
    pub(crate) fn get(&self, nodes: &[Node], tree: Option<u64>, changed: bool) -> Rc<Preparation> {
        let snapshot = self.snapshot.take();
        let fresh = tree.is_some() && !changed;
        let key = tree.unwrap_or(BASE);
        let prepared = match snapshot {
            Some((old, prepared)) if fresh && old == key => prepared,
            Some((BASE, base)) if fresh => Rc::new(Preparation::continued(&base, nodes)),
            _ => Rc::new(Preparation::new(nodes)),
        };
        if tree.is_some() {
            self.snapshot.set(Some((key, prepared.clone())));
        }
        prepared
    }
}

impl Preparation {
    /// Подготовка дерева той же формы и стилей, что у `base`. Неизменённые
    /// COW-списки уже владеют фактами; обрабатываются только скопированные.
    fn continued(base: &Self, nodes: &[Node]) -> Self {
        let mut facts = SubtreeFacts::default();
        let clearance_possible = facts.clearance_possible(nodes);
        Self {
            unkeyed: base.unkeyed.clone(),
            clearance_possible,
        }
    }

    fn new(nodes: &[Node]) -> Self {
        crate::work::add!(FullTreeWalks, 1);
        let mut facts = SubtreeFacts::default();
        let mut map = std::collections::HashMap::new();
        let mut at = 0;
        let mut last = None;
        let mut pending: Vec<(&Element, bool)> = nodes
            .iter()
            .rev()
            .filter_map(|node| {
                if let Node::Element(e) = node {
                    Some((e, false))
                } else {
                    None
                }
            })
            .collect();
        while let Some((e, leave)) = pending.pop() {
            if leave {
                let flags = facts.list_flags(&e.children);
                e.children.publish_box_summary(flags);
                // Конец поддерева исключает собственных потомков из проверки
                // более позднего позиционированного (CSS2 Appendix E, шаг 8).
                map.insert(e.node_id, at);
            } else {
                crate::work::add!(TreeNodesVisited, 1);
                if unkeyed_positioned(e) {
                    last = Some(at);
                }
                at += 1;
                pending.push((e, true));
                for node in e.children.iter().rev() {
                    if let Node::Element(child) = node {
                        pending.push((child, false));
                    }
                }
            }
        }
        let clearance_possible = facts.clearance_possible(nodes);
        Self {
            unkeyed: Rc::new((map, last)),
            clearance_possible,
        }
    }
}

#[cfg(test)]
mod tests;
