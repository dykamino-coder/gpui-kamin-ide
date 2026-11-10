//! Записи блоков одного документа между кадрами (`Store`).

use super::{FlowKey, Record, narrow, replay::ReplayRecord};
use crate::dom::Element;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    /// Записи под ключом `last`: другой ключ их не принимает, поэтому смена
    /// ключа их сбрасывает, а не держит устаревшие стили.
    pub(super) records: RefCell<HashMap<u64, Rc<Record>>>,
    /// Records of blocks replayed on screen with their layout subtree
    /// (`replay`), by node id.
    pub(super) replays: RefCell<HashMap<u64, Rc<ReplayRecord>>>,
    /// Blocks whose replay failed the frame check (moved, or their layout
    /// needed a new measurement): they are built until the flow key changes.
    pub(super) banned: RefCell<HashSet<u64>>,
    /// Blocks whose last replay recording read a width from outside: their
    /// records fit only the same widths.
    pub(super) reads_widths: RefCell<HashSet<u64>>,
    /// Blocks whose last skip wrap (`recorder::Pending`) ended without a
    /// record: a build at a changed width wraps them for subtree replay too.
    pub(super) unrecorded: RefCell<HashSet<u64>>,
    /// Ключ потока прошлой сборки: записи делаются, только пока он устоялся,
    /// иначе (ширина меняется каждый кадр) запись не окупается.
    pub(super) last: RefCell<Option<Rc<FlowKey>>>,
    /// Ключ кадра до `last`: повторная сборка того же кадра сравнивает с
    /// ним, как первая (`FlowReuse::again`).
    pub(super) prev: RefCell<Option<Rc<FlowKey>>>,
    /// Прошлая сборка потока была устоявшейся (ключ равен предыдущему).
    pub(super) settled: Cell<bool>,
    /// Первая сборка кадра нашла поток тёплым (`FlowCx::warm`): повторная
    /// сборка того же кадра берёт это, а не свой ответ.
    pub(super) warm: Cell<bool>,
    /// Blocks whose text wrapped: not recorded in a flow no wider (`narrow`).
    pub(super) narrow: narrow::Wraps,
    /// Блоки, отказанные `FlowReuse::validate`: пересборка кадра их собирает.
    pub(super) refused: RefCell<HashSet<u64>>,
    /// Номер кадра prepaint документа (`draw`): диапазоны повтора записи
    /// годны только в кадре сразу после своего.
    pub(super) epoch: Cell<u64>,
    /// Наблюдение для тестов и стенда: записано, пропущено, кадров заново.
    pub(crate) stats: Cell<ReuseStats>,
    /// The document was patched since the last frame (`Document::commit`):
    /// the records are known to hold only by the next build's comparison.
    pub(super) patched: Cell<bool>,
}

impl Store {
    /// Начался prepaint документа; `continued` — его состояние элемента было
    /// в прошлом кадре окна, то есть прошлый кадр рисовал этот документ.
    /// Иначе цепочка кадров рвётся, и ни один диапазон повтора не годен.
    pub(crate) fn draw(&self, continued: bool) {
        self.epoch
            .set(self.epoch.get() + if continued { 1 } else { 2 });
    }

    /// Кадр документа повторён целиком: диапазоны записей повтора блоков
    /// указывают в кадр до него и больше не годятся.
    pub(crate) fn forget_replays(&self) {
        self.replays.take();
    }

    /// Запись блока `e`, если его поддерево то же, что при записи. Запись
    /// изменённого блока (`Document::update`) не подойдёт ему никогда, а держит
    /// прежнее поддерево (узлы, стили) всю сборку нового: она удаляется сразу,
    /// до сборки блока, а не в конце кадра.
    ///
    /// A block whose subtree differs only in paint-only style values
    /// (`Computed::same_layout`, Blink paint invalidation without layout) keeps
    /// its record for its layout: the answer is then `Damage::Paint`, and only
    /// the recorded layout leaf, not the recorded paint, holds.
    pub(crate) fn same_tree(&self, e: &Element) -> Option<(Rc<Record>, Damage)> {
        let record = self.records.borrow().get(&e.node_id).cloned()?;
        let damage = match record.repaint {
            true => Damage::Paint,
            false => Damage::None,
        };
        // Compared at the rebuild of the document (`rebase`).
        let Some(source) = &record.source else {
            return Some((record, damage));
        };
        if source.same(e) {
            return Some((record, damage));
        }
        // A repainted block stands only off screen (`FlowCx::placeholder`):
        // the layout compare of a near one, a walk of its whole subtree,
        // would only find a record its build cannot use.
        if record.far && source.same_layout(e) {
            // The caller keeps the layout under a record of this tree
            // (`FlowCx::placeholder`): the previous subtree goes now, not at
            // the end of the frame.
            let mut records = self.records.take();
            records.remove(&e.node_id);
            self.records.replace(records);
            return Some((record, Damage::Paint));
        }
        // Записи держат стили: карта меняется целиком, без `&mut` на них.
        let mut records = self.records.take();
        records.remove(&e.node_id);
        self.records.replace(records);
        None
    }

    /// Build inputs of the kept records, for tests.
    #[cfg(test)]
    pub(crate) fn recorded(&self) -> Vec<Element> {
        let records = self.records.borrow();
        let sources = records.values().filter_map(|r| r.source.as_ref());
        sources.map(|s| s.element()).collect()
    }

    /// The document `old` was rebuilt as `new` from the same source DOM
    /// (`Document::apply`, same-shape `Document::update`): the node ids are
    /// the same. Each record's block is compared in the two trees by its
    /// cascaded values (`dom::same_cascaded_tree`: both computed from them
    /// under the root boxes, whose layout values the flow key compares), and
    /// the record drops its source, a block of the previous frame tree, which
    /// then goes with the previous document, as for a fresh one, instead of
    /// living into the next build. A block is equal only if the `html`/`body`
    /// boxes cascade equal values too (their paint-only values reach it as
    /// its own); a block equal but for paint-only values keeps its record only
    /// if it was far from the screen (`Damage::Paint` stands only there).
    /// Replay records, which hold their output, go. After a patch since the
    /// last frame (`patch`), the records hold only by the next build's
    /// comparison.
    pub(crate) fn rebase(&self, old: &[crate::dom::Node], new: &[crate::dom::Node]) {
        if self.patched.get() {
            return;
        }
        self.reset_replays();
        let mut records = self.records.take();
        let ((was, a), (now, b)) = (super::flow_blocks(old), super::flow_blocks(new));
        let roots = match (a, b) {
            (Some(a), Some(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(&b)
                        .all(|(x, y)| crate::dom::same_cascaded_box(x, y, false))
            }
            _ => false,
        };
        // The same source DOM: the flows have the same children in the same
        // order, so the blocks pair by position (no map of either tree).
        let mut kept = HashMap::with_capacity(records.len());
        let pairs = was.iter().zip(now).filter_map(|pair| match pair {
            (crate::dom::Node::Element(a), crate::dom::Node::Element(b)) => Some((a, b)),
            _ => None,
        });
        for (a, b) in pairs.take_while(|_| !records.is_empty()) {
            let Some(record) = records.remove(&a.node_id) else {
                continue;
            };
            let exact = roots && crate::dom::same_cascaded_tree(a, b, false);
            let repaint = !exact && record.far && crate::dom::same_cascaded_tree(a, b, true);
            if exact || repaint {
                kept.insert(a.node_id, Rc::new(record.moved_to(None, repaint)));
            }
        }
        self.records.replace(kept);
    }

    /// The document was patched (`Document::commit`): records compared at a
    /// rebuild (`rebase`) no longer hold, the others are compared by the
    /// next build.
    pub(crate) fn patch(&self) {
        self.patched.set(true);
        let mut records = self.records.take();
        records.retain(|_, r| r.source.is_some());
        self.records.replace(records);
    }

    /// Ширина потока сменилась: годны только записи, чья раскладка от неё
    /// не зависит.
    pub(super) fn keep_width_independent(&self, key: &FlowKey) {
        // Записи держат стили: карта меняется целиком, без `&mut` на них.
        let mut records = self.records.take();
        records.retain(|_, r| r.fits_width(key));
        self.records.replace(records);
    }

    /// The flow key changed in more than its width: no record or wrap of
    /// the last key holds.
    pub(super) fn reset(&self) {
        self.records.take();
        self.narrow.reset();
    }

    /// The flow key changed outside its widths: no replay record holds.
    pub(super) fn reset_replays(&self) {
        self.replays.take();
        self.banned.take();
        self.reads_widths.take();
    }

    pub(super) fn banned(&self, node: u64) -> bool {
        self.banned.borrow().contains(&node)
    }

    pub(super) fn reads_widths(&self, node: u64) -> bool {
        self.reads_widths.borrow().contains(&node)
    }

    pub(super) fn unrecorded(&self, node: u64) -> bool {
        self.unrecorded.borrow().contains(&node)
    }
}

/// What changed in a recorded block since its record (`Store::same_tree`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Damage {
    /// Nothing: the recorded layout and paint hold.
    None,
    /// Paint-only values: the recorded layout holds, its paint does not.
    Paint,
}

/// Счётчики наблюдения повтора блоков.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReuseStats {
    /// Blocks wrapped for recording (`recorder::Pending`).
    pub tracked: u64,
    pub recorded: u64,
    pub skipped: u64,
    pub rebuilt_frames: u64,
    /// Blocks of the root flow replayed on screen without a build: skipped
    /// blocks whose output of the last frame was replayed, and blocks whose
    /// layout subtree was claimed whole (`replay`).
    pub replayed: u64,
    /// Of `replayed`, blocks whose layout subtree was claimed whole.
    pub subtrees: u64,
    /// Blocks wrapped for a subtree replay record (`replay::PendingReplay`).
    pub replay_tracked: u64,
}
