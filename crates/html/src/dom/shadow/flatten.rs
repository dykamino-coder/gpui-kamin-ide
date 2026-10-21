//! Плоское дерево слотов: «find flattened slottables» и выкладка на склад.
//!
//! `flatten_slot` раскрывает распределённые узлы слота, а без них — его
//! fallback-детей; слот среди них раскрывается рекурсивно (глубина ограничена
//! стражем). `finish_slots` досчитывает плоские списки всех черновиков,
//! кладёт слоты в `TreeIndex::slots` (их читает обход `dom::walk`) и
//! регистрирует правила `::slotted` (`slotted::register_all`).

use super::{FlatCtx, SlotInfo, is_slot};
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::style::select::census_of;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Плоские распределённые слота (DOM «find flattened slottables»):
/// распределённые, иначе fallback-дети; слот среди них раскрывается
/// рекурсивно. Глубина ограничена: цикла распределений в дереве быть не
/// может, но стража дешевле доказательства.
pub(super) fn flatten_slot(
    key: &NodeKey,
    drafts: &HashMap<NodeKey, SlotInfo>,
    tree: &TreeIndex,
    depth: usize,
    out: &mut Vec<Option<FlatCtx>>,
) {
    let Some(info) = drafts.get(key) else { return };
    if depth > 32 {
        return;
    }
    if !info.assigned.is_empty() {
        for s in &info.assigned {
            let nested = NodeKey::of(&s.node);
            if is_slot(&s.node) && drafts.contains_key(&nested) {
                flatten_slot(&nested, drafts, tree, depth + 1, out);
            } else {
                out.push(s.anc.clone().map(|anc| FlatCtx {
                    node: s.node.clone(),
                    anc,
                    path: info.host_path.clone(),
                    all: info.light_all.clone(),
                    pos: s.elem_pos,
                }));
            }
        }
        return;
    }
    let kids: Vec<Handle> = info.slot.children.borrow().clone();
    let (_, all) = census_of(&kids, tree);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for kid in &kids {
        match &kid.data {
            NodeData::Element { .. } => {
                let nested = NodeKey::of(kid);
                if is_slot(kid) && drafts.contains_key(&nested) {
                    flatten_slot(&nested, drafts, tree, depth + 1, out);
                } else {
                    out.push(Some(FlatCtx {
                        node: kid.clone(),
                        anc: all[pos].clone(),
                        path: Rc::new(vec![]),
                        all: all.clone(),
                        pos,
                    }));
                }
                pos += 1;
            }
            NodeData::Text { .. } => out.push(None),
            _ => {}
        }
    }
}

/// Досчитать плоские списки и выложить слоты на склад.
pub(crate) fn finish_slots(mut drafts: HashMap<NodeKey, SlotInfo>, tree: &mut TreeIndex) {
    let keys: Vec<NodeKey> = drafts.keys().cloned().collect();
    let mut flats: HashMap<NodeKey, Vec<Option<FlatCtx>>> = HashMap::new();
    for key in &keys {
        let mut flat = vec![];
        flatten_slot(key, &drafts, tree, 0, &mut flat);
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
