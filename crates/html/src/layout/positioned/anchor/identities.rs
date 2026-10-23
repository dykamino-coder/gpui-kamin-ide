//! Явные идентичности исходного дерева при отложенном построении OOF.

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct Identities(HashMap<u64, (u64, u32)>);

fn source_key(e: &Element) -> Option<u64> {
    // Анонимные коробки не имеют исходной идентичности. Их общий anchor key
    // нельзя использовать как ключ краски нескольких разных коробок.
    (e.node_id != 0 || e.style.computed().implicit_anchor.is_some()).then(|| super::key_of(e))
}

impl LayoutCx {
    pub(crate) fn paint_key_for(&self, e: &Element) -> u64 {
        self.oof_identities
            .as_ref()
            .and_then(|ids| source_key(e).and_then(|key| ids.0.get(&key)))
            .map(|id| id.0)
            .unwrap_or_else(|| self.next_paint_key())
    }

    pub(crate) fn anchor_seq_for(&self, e: &Element) -> u32 {
        self.oof_identities
            .as_ref()
            .and_then(|ids| source_key(e).and_then(|key| ids.0.get(&key)))
            .map(|id| id.1)
            .unwrap_or_else(|| super::next_seq(self.vcx()))
    }

    pub(crate) fn retain_oof_identities(&self, e: &Element, paint_key: u64) -> Rc<Identities> {
        let mut ids = self.oof_identities.as_deref().cloned().unwrap_or_default();
        fn visit(cx: &LayoutCx, e: &Element, paint: Option<u64>, ids: &mut Identities) {
            if let Some(key) = source_key(e) {
                ids.0.entry(key).or_insert_with(|| {
                    (
                        paint.unwrap_or_else(|| cx.next_paint_key()),
                        super::next_seq(cx.vcx()),
                    )
                });
            }
            for child in &e.children {
                if let Node::Element(child) = child {
                    visit(cx, child, None, ids);
                }
            }
        }
        visit(self, e, Some(paint_key), &mut ids);
        Rc::new(ids)
    }
}
