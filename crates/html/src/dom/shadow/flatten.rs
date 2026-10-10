//! Flatten for shadow; split out to keep the owning module within 250 lines.

use super::{FlatCtx, SlotInfo, is_slot, node_key};
use crate::dom::*;
use crate::style::select::census_of;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

/// Плоские распределённые слота (DOM «find flattened slottables»):
/// распределённые, иначе fallback-дети; слот среди них раскрывается
/// рекурсивно. Глубина ограничена: цикла распределений в дереве быть не
/// может, но стража дешевле доказательства.
pub(super) fn flatten_slot(
    key: usize,
    drafts: &HashMap<usize, SlotInfo>,
    depth: usize,
    out: &mut Vec<Option<FlatCtx>>,
) {
    let Some(info) = drafts.get(&key) else { return };
    if depth > 32 {
        return;
    }
    if !info.assigned.is_empty() {
        for s in &info.assigned {
            if is_slot(&s.node) && drafts.contains_key(&node_key(&s.node)) {
                flatten_slot(node_key(&s.node), drafts, depth + 1, out);
            } else {
                out.push(s.anc.clone().map(|anc| FlatCtx {
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
    let (_, all) = census_of(&kids);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for kid in &kids {
        match &kid.data {
            NodeData::Element { .. } => {
                if is_slot(kid) && drafts.contains_key(&node_key(kid)) {
                    flatten_slot(node_key(kid), drafts, depth + 1, out);
                } else {
                    out.push(Some(FlatCtx {
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
pub(crate) fn finish_slots(mut drafts: HashMap<usize, SlotInfo>) {
    let keys: Vec<usize> = drafts.keys().copied().collect();
    let mut flats: HashMap<usize, Vec<Option<FlatCtx>>> = HashMap::new();
    for key in &keys {
        let mut flat = vec![];
        flatten_slot(*key, &drafts, 0, &mut flat);
        flats.insert(*key, flat);
    }
    SLOTS.with(|m| {
        let mut m = m.borrow_mut();
        for key in keys {
            if let Some(mut info) = drafts.remove(&key) {
                info.flattened = flats.remove(&key).unwrap_or_default();
                m.insert(key, Rc::new(info));
            }
        }
    });
}
