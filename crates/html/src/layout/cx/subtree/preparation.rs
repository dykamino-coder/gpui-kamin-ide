//! Подготовка снимка документа: факты post-order и индексы краски одним обходом.

use super::SubtreeFacts;
use crate::dom::{Element, Node};
use crate::layout::order::{Unkeyed, unkeyed_positioned};
use std::rc::Rc;

pub(crate) struct Preparation {
    pub(crate) facts: Rc<SubtreeFacts>,
    pub(crate) unkeyed: Rc<Unkeyed>,
    pub(crate) clearance_possible: bool,
    /// Сколько подготовок продолжено от последней полной (`continued`).
    depth: u8,
}

/// Предел цепочки продолжений: глубже факты собираются полным обходом, чтобы
/// поиск по слоям и удержанные ими метки снимков не росли с числом правок.
const MAX_DEPTH: u8 = 8;

type Snapshot = ((usize, usize), Rc<Preparation>);

#[derive(Default)]
pub(crate) struct PreparationCache {
    snapshot: std::cell::Cell<Option<Snapshot>>,
    /// Подготовка дерева прошлой ревизии той же формы и стилей (`continuing`).
    base: std::cell::Cell<Option<Rc<Preparation>>>,
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
        let base = match &snapshot {
            Some((_, prepared)) => Some(prepared.clone()),
            None => {
                let base = previous.base.take();
                previous.base.set(base.clone());
                base
            }
        };
        previous.snapshot.set(snapshot);
        Self {
            snapshot: Default::default(),
            base: std::cell::Cell::new(base),
        }
    }

    /// Только Document гарантирует неизменность корневого списка. Для raw
    /// render адрес не доказывает неизменность элементов и кэш не применяется.
    pub(crate) fn get(&self, nodes: &[Node], stable: bool, changed: bool) -> Rc<Preparation> {
        let key = (nodes.as_ptr() as usize, nodes.len());
        let snapshot = self.snapshot.take();
        let prepared = match snapshot {
            Some((old, prepared)) if stable && !changed && old == key => prepared,
            _ => match self.base.take() {
                Some(base) if stable && !changed && base.depth < MAX_DEPTH => {
                    Rc::new(Preparation::continued(&base, nodes))
                }
                _ => Rc::new(Preparation::new(nodes)),
            },
        };
        if stable {
            self.snapshot.set(Some((key, prepared.clone())));
        }
        prepared
    }
}

impl Preparation {
    /// Подготовка дерева той же формы и стилей, что у `base`: факты — слой
    /// над фактами `base` с записями только для новых списков детей.
    fn continued(base: &Self, nodes: &[Node]) -> Self {
        let mut facts = SubtreeFacts::overlay(base.facts.clone());
        let clearance_possible = facts.clearance_possible(nodes);
        Self {
            facts: Rc::new(facts),
            unkeyed: base.unkeyed.clone(),
            clearance_possible,
            depth: base.depth + 1,
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
                facts
                    .children
                    .insert(e.children.identity(), (e.children.mark(), flags));
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
            facts: Rc::new(facts),
            unkeyed: Rc::new((map, last)),
            clearance_possible,
            depth: 0,
        }
    }
}

#[cfg(test)]
mod tests;
