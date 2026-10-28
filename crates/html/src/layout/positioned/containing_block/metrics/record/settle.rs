//! Установка якорных коробок до снимка: якорная коробка встаёт на prepaint, когда
//! якоря опубликованы (css-anchor-position-1 §3), сдвигом от своего места в
//! раскладке. Проход установки (`layout::source`) собирает итоговые места, и снимок
//! ставит коробку, её семью и вложенные потоки туда, где их нарисует источник.

use super::{Entry, Recorder};
use gpui::{Pixels, Point, Window};
use std::{collections::HashMap, rc::Rc};

/// Prepaint context of a settling pass: where each anchored box was placed.
pub(crate) struct Settling(pub(crate) Rc<Recorder>);

/// Report the settled placement (window origin) of anchored box `key` to a
/// settling pass, if one runs.
pub(crate) fn settled_offset(key: u64, placed: Point<Pixels>, window: &Window) {
    if let Some(settling) = window.render_context::<Settling>() {
        settling.0.offsets.borrow_mut().insert(key, placed);
    }
}

impl Recorder {
    /// Displacement from layout to settled placement of every flow at or inside
    /// an anchor-positioned flow, by flow id: an anchored box moves to where the
    /// pass placed it, a box inside it moves with it.
    pub(super) fn settled(
        &self,
        nodes: &HashMap<u64, Entry>,
        window: &Window,
    ) -> HashMap<u64, Point<Pixels>> {
        let placed = self.offsets.borrow();
        let held = self.held();
        let mut settled: HashMap<u64, Point<Pixels>> = HashMap::new();
        for (id, parent) in self.flows.settling() {
            if held.contains(&id) {
                continue;
            }
            let outer = parent
                .and_then(|parent| settled.get(&parent).copied())
                .unwrap_or_default();
            let own = placed
                .get(&id)
                .zip(nodes.get(&id).and_then(|e| e.node.read(window)))
                .map(|(at, layout)| *at - layout.normal.origin - layout.relative_offset);
            settled.insert(id, own.unwrap_or(outer));
        }
        settled
    }

    /// Flows at or inside an anchored box the settling pass could not anchor
    /// (its anchor lives in another source): its copies keep painting them.
    pub(crate) fn held(&self) -> std::collections::HashSet<u64> {
        let unsettled = self.unsettled.borrow();
        let mut held = std::collections::HashSet::new();
        for (id, parent) in self.flows.settling() {
            if unsettled.contains(&id) || parent.is_some_and(|p| held.contains(&p)) {
                held.insert(id);
            }
        }
        held
    }
}
