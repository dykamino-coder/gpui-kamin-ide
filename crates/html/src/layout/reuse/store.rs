//! Записи повтора блоков одного документа между кадрами (`Store`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    /// Записи под ключом `last`: другой ключ их не принимает, поэтому смена
    /// ключа их сбрасывает, а не держит устаревшие стили.
    pub(super) records: RefCell<HashMap<u64, Rc<super::Record>>>,
    /// Records of blocks replayed on screen (`replay`), by node id.
    pub(super) replays: RefCell<HashMap<u64, Rc<super::replay::ReplayRecord>>>,
    /// Blocks whose replay failed the frame check (moved, or their layout
    /// needed a new measurement): they are built until the flow key changes.
    pub(super) banned: RefCell<std::collections::HashSet<u64>>,
    /// Blocks whose last replay recording read a width from outside: their
    /// records fit only the same widths.
    pub(super) reads_widths: RefCell<std::collections::HashSet<u64>>,
    /// Ключ потока прошлой сборки: записи делаются, только пока он устоялся,
    /// иначе (ширина меняется каждый кадр) запись не окупается.
    pub(super) last: RefCell<Option<Rc<super::FlowKey>>>,
    /// Прошлая сборка потока была устоявшейся (ключ равен предыдущему).
    pub(super) settled: Cell<bool>,
    /// Наблюдение для тестов и стенда: записано, пропущено, кадров заново.
    pub(crate) stats: Cell<ReuseStats>,
}

impl Store {
    /// Кадр документа повторён целиком: диапазоны записей повтора блоков
    /// указывают в кадр до него и больше не годятся.
    pub(crate) fn forget_replays(&self) {
        self.replays.take();
    }

    pub(super) fn banned(&self, node: u64) -> bool {
        self.banned.borrow().contains(&node)
    }

    pub(super) fn reads_widths(&self, node: u64) -> bool {
        self.reads_widths.borrow().contains(&node)
    }
}

/// Счётчики наблюдения повтора блоков.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReuseStats {
    pub recorded: u64,
    pub skipped: u64,
    pub rebuilt_frames: u64,
    /// Blocks replayed on screen without a build (`replay`).
    pub replayed: u64,
}
