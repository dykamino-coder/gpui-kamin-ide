//! Записи блоков одного документа между кадрами (`Store`).

use super::{FlowKey, Record, far::FarRecord, narrow, replay::ReplayRecord};
use crate::dom::Element;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Far records by node id (`Store::far`).
pub(super) type FarRecords = HashMap<u64, Rc<FarRecord>>;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    /// Записи под ключом `last`: другой ключ их не принимает, поэтому смена
    /// ключа их сбрасывает, а не держит устаревшие стили.
    pub(super) records: RefCell<HashMap<u64, Rc<Record>>>,
    /// Records of blocks replayed on screen with their layout subtree
    /// (`replay`), by node id.
    pub(super) replays: RefCell<HashMap<u64, Rc<ReplayRecord>>>,
    /// Records of far blocks laid out from their kept layout subtree
    /// (`far`), by node id; allocated while there are any.
    pub(super) far: Cell<Option<Box<FarRecords>>>,
    /// The last blocks whose far recording failed (near the screen, not
    /// quiet or reading an outside width; 0 is empty): they are not wrapped
    /// again while the flow key holds outside the widths, so a block on
    /// screen costs no far wrapper per frame. Two slots cover the single
    /// on-screen root block of a page; more near blocks just record again.
    pub(super) far_near: Cell<[u64; 2]>,
    /// Blocks whose replay failed the frame check (moved, or their layout
    /// needed a new measurement): they are built until the flow key changes.
    pub(super) banned: Cell<HashSet<u64>>,
    /// Blocks whose last replay recording read a width from outside: their
    /// records fit only the same widths.
    pub(super) reads_widths: Cell<HashSet<u64>>,
    /// Blocks whose last skip wrap (`recorder::Pending`) ended without a
    /// record: a build at a changed width wraps them for subtree replay too.
    pub(super) unrecorded: Cell<HashSet<u64>>,
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
    pub(super) refused: Cell<HashSet<u64>>,
    /// Номер кадра prepaint документа (`draw`): диапазоны повтора записи
    /// годны только в кадре сразу после своего.
    pub(super) epoch: Cell<u64>,
    /// Наблюдение для тестов и стенда: записано, пропущено, кадров заново.
    pub(crate) stats: Cell<ReuseStats>,
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
    /// The layout pass of the replayed frame did not request the document's
    /// nodes either, so no kept layout subtree is left (`far`).
    pub(crate) fn forget_replays(&self) {
        self.replays.take();
        self.far.take();
    }

    /// Запись блока `e`, если его поддерево то же, что при записи. Запись
    /// изменённого блока (`Document::update`) не подойдёт ему никогда, а держит
    /// прежнее поддерево (узлы, стили) всю сборку нового: она удаляется сразу,
    /// до сборки блока, а не в конце кадра.
    pub(crate) fn same_tree(&self, e: &Element) -> Option<Rc<Record>> {
        let record = self.records.borrow().get(&e.node_id).cloned()?;
        if record.source.same(e) {
            return Some(record);
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
        records.values().map(|r| r.source.element()).collect()
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
        self.far.take();
        self.far_near.take();
        self.banned.take();
        self.reads_widths.take();
    }

    pub(super) fn banned(&self, node: u64) -> bool {
        let banned = self.banned.take();
        let found = banned.contains(&node);
        self.banned.set(banned);
        found
    }

    /// The far record of block `node`.
    pub(super) fn far_record(&self, node: u64) -> Option<Rc<FarRecord>> {
        let far = self.far.take();
        let record = far.as_ref().and_then(|far| far.get(&node).cloned());
        self.far.set(far);
        record
    }

    /// The last far recording of block `node` failed (`far_near`).
    pub(super) fn far_near(&self, node: u64) -> bool {
        node != 0 && self.far_near.get().contains(&node)
    }

    pub(super) fn note_near(&self, node: u64) {
        let [a, b] = self.far_near.get();
        if node != 0 && a != node && b != node {
            self.far_near.set([node, a]);
        }
    }

    pub(super) fn forget_near(&self, node: u64) {
        let near = self.far_near.get().map(|n| if n == node { 0 } else { n });
        self.far_near.set(near);
    }

    pub(super) fn reads_widths(&self, node: u64) -> bool {
        let reads = self.reads_widths.take();
        let found = reads.contains(&node);
        self.reads_widths.set(reads);
        found
    }

    pub(super) fn refused(&self, node: u64) -> bool {
        let refused = self.refused.take();
        let found = refused.contains(&node);
        self.refused.set(refused);
        found
    }

    pub(super) fn unrecorded(&self, node: u64) -> bool {
        let unrecorded = self.unrecorded.take();
        let found = unrecorded.contains(&node);
        self.unrecorded.set(unrecorded);
        found
    }
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
    /// Far blocks laid out from their kept layout subtree without a build
    /// (`far`).
    pub far: u64,
}
