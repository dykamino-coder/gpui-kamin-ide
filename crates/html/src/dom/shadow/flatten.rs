//! Плоское дерево слотов: «find flattened slottables» и выкладка на склад.
//!
//! `flatten_slot` раскрывает распределённые узлы слота, а без них — его
//! fallback-детей; слот среди них раскрывается явным стеком продолжений. `finish_slots` досчитывает плоские списки всех черновиков,
//! кладёт слоты в `TreeIndex::slots` (их читает обход `dom::walk`) и
//! регистрирует правила `::slotted` (`slotted::register_all`).

use super::{FlatCtx, SlotInfo, is_slot};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::select::census_of;
use markup5ever_rcdom::NodeData;
use std::collections::HashMap;
use std::rc::Rc;

/// Раскрытие слотов в порядке дерева без зависимости от стека потока.
/// Распределение следует потомкам DOM, поэтому циклов здесь нет.
pub(super) fn flatten_slot(
    key: &NodeKey,
    drafts: &HashMap<NodeKey, SlotInfo>,
    tree: &TreeIndex,
    out: &mut Vec<Option<FlatCtx>>,
) {
    enum Work {
        Expand(NodeKey),
        Emit(Option<FlatCtx>),
    }
    let mut work = vec![Work::Expand(key.clone())];
    while let Some(next) = work.pop() {
        let key = match next {
            Work::Emit(ctx) => {
                out.push(ctx);
                continue;
            }
            Work::Expand(key) => key,
        };
        let Some(info) = drafts.get(&key) else {
            continue;
        };
        if !info.assigned.is_empty() {
            for s in info.assigned.iter().rev() {
                let nested = NodeKey::of(&s.node);
                if is_slot(&s.node) && drafts.contains_key(&nested) {
                    work.push(Work::Expand(nested));
                } else {
                    work.push(Work::Emit(s.anc.clone().map(|anc| FlatCtx {
                        node: s.node.clone(),
                        anc,
                        path: info.host_path.clone(),
                        all: info.light_all.clone(),
                        pos: s.elem_pos,
                    })));
                }
            }
            continue;
        }
        let kids = info.slot.children.borrow();
        let (_, all) = census_of(&kids, tree);
        let all = Rc::new(all);
        let mut pos = all.len();
        for kid in kids.iter().rev() {
            match &kid.data {
                NodeData::Element { .. } => {
                    pos -= 1;
                    let nested = NodeKey::of(kid);
                    if is_slot(kid) && drafts.contains_key(&nested) {
                        work.push(Work::Expand(nested));
                    } else {
                        work.push(Work::Emit(Some(FlatCtx {
                            node: kid.clone(),
                            anc: all[pos].clone(),
                            path: Rc::new(vec![]),
                            all: all.clone(),
                            pos,
                        })));
                    }
                }
                NodeData::Text { .. } => work.push(Work::Emit(None)),
                _ => {}
            }
        }
    }
}

/// Досчитать плоские списки и выложить слоты на склад.
pub(crate) fn finish_slots(mut drafts: HashMap<NodeKey, SlotInfo>, tree: &mut TreeIndex) {
    let keys: Vec<NodeKey> = drafts.keys().cloned().collect();
    let mut flats: HashMap<NodeKey, Vec<Option<FlatCtx>>> = HashMap::new();
    for key in &keys {
        let mut flat = vec![];
        flatten_slot(key, &drafts, tree, &mut flat);
        flats.insert(key.clone(), flat);
    }
    for key in keys {
        if let Some(mut info) = drafts.remove(&key) {
            info.flattened = flats.remove(&key).unwrap_or_default();
            tree.slots.insert(key, Rc::new(info));
        }
    }
    super::slotted::register_all(tree);
}
