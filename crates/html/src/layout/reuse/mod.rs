//! Пропуск устоявшихся блоков корневого потока документа вне экрана.
//!
//! Корневой поток — дети `body`, а если стандартный документ снял обёртки
//! `html`/`body` (`document::embedded::unwrap_document`), то сами блоки
//! верхнего уровня. Блочный ребёнок потока, чьё поддерево после проходов
//! подготовки равно по значениям записанному (`dom::same_tree`) и чей поток
//! имеет тот же ключ (`FlowKey`: стиль корня, коробки `html`/`body`,
//! ограничения, опции хоста, поколение шрифтовых входов), раскладывается так
//! же, как в кадре записи (Blink `LayoutResult` cache, Taffy `Cache`:
//! результат по ключу пространства ограничений). Такой ребёнок не
//! собирается: на его месте точный лист раскладки (`gpui::LayoutSnapshot`)
//! того же размера и полей. Пропуск допустим, только если ни один его
//! примитив не попал бы на экран: записанные чернила (`gpui::InkSink`, с
//! отсечёнными) на новом месте проверяются до prepaint кадра, и любой промах
//! собирает кадр заново без заместителей (`FlowReuse::validate`).
//! Записываются только инертные поддеревья: обычный поток без
//! позиционирования, флоатов и эффектов, без hitbox, состояния элементов,
//! ввода и отложенных отрисовок GPUI, и сборка которых меняет лишь счётчики
//! ключей краски и якорей. Запись не копирует поддерево: элемент делит детей
//! и стили с деревом кадра, а лист — несколько чисел.

mod flow;
mod key;
mod placeholder;
mod plain;
mod recorder;
mod replay;
#[cfg(test)]
mod tests;

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use gpui::{Bounds, LayoutSnapshot, ScaledPixels, Window, point, px, size};
pub(crate) use key::{Counters, FlowKey};
use recorder::{SNAP, placed, scaled};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    /// Записи под ключом `last`: другой ключ их не принимает, поэтому смена
    /// ключа их сбрасывает, а не держит устаревшие стили.
    records: RefCell<HashMap<u64, Rc<Record>>>,
    /// Records of blocks replayed on screen (`replay`), by node id.
    replays: RefCell<HashMap<u64, Rc<replay::ReplayRecord>>>,
    /// Blocks whose replay failed the frame check (moved, or their layout
    /// needed a new measurement): they are built until the flow key changes.
    banned: RefCell<std::collections::HashSet<u64>>,
    /// Ключ потока прошлой сборки: записи делаются, только пока он устоялся,
    /// иначе (ширина меняется каждый кадр) запись не окупается.
    last: RefCell<Option<Rc<FlowKey>>>,
    /// Прошлая сборка потока была устоявшейся (ключ равен предыдущему).
    settled: Cell<bool>,
    /// Наблюдение для тестов и стенда: записано, пропущено, кадров заново.
    pub(crate) stats: Cell<ReuseStats>,
}

impl Store {
    /// Кадр документа повторён целиком: диапазоны записей повтора блоков
    /// указывают в кадр до него и больше не годятся.
    pub(crate) fn forget_replays(&self) {
        self.replays.take();
    }

    fn banned(&self, node: u64) -> bool {
        self.banned.borrow().contains(&node)
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

/// Запись блока: вход сборки и то, что нужно заместителю и проверке экрана.
/// Пишется только для блока, бывшего далеко от экрана (`recorder::far`):
/// запись ближнего блока не подошла бы ни одному кадру.
pub(crate) struct Record {
    /// Вход сборки блока; одна копия на блок, общая с записью повтора.
    element: Rc<Element>,
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    snapshot: LayoutSnapshot,
    /// Чернила относительно точного начала коробки; `None` — ничего не рисовал.
    ink: Option<Bounds<ScaledPixels>>,
}

/// Какой поток документа корневой: дети `body` или блоки верхнего уровня.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Root {
    Body { html: Option<u64>, body: u64 },
    TopLevel,
}

/// Повтор блоков одной сборки документа.
pub(crate) struct FlowReuse {
    store: Rc<Store>,
    /// Заместители разрешены; иначе только запись.
    allow: bool,
    doc: (usize, usize),
    root: Root,
    seen: (OnceCell<Element>, OnceCell<Element>),
    claimed: Cell<bool>,
    next: RefCell<HashMap<u64, Rc<Record>>>,
    /// Ожидающие записи с элементами своих блоков.
    pending: RefCell<Vec<(Rc<recorder::Pending>, Rc<Element>)>>,
    placeholders: RefCell<Vec<Rc<placeholder::Slot>>>,
    next_replays: RefCell<HashMap<u64, Rc<replay::ReplayRecord>>>,
    pending_replays: RefCell<Vec<(Rc<replay::PendingReplay>, Rc<Element>)>>,
    replay_slots: RefCell<Vec<Rc<replay::ReplaySlot>>>,
    screen: Cell<Option<Bounds<ScaledPixels>>>,
}

fn find<'a>(nodes: impl Iterator<Item = &'a Node>, tag: &str) -> Option<&'a Element> {
    nodes.into_iter().find_map(|n| match n {
        Node::Element(e) if e.tag == tag => Some(e),
        _ => None,
    })
}

impl FlowReuse {
    /// Повтор для сборки дерева `nodes`: корень `html` с `body`, `body` без
    /// снятого `html` или блоки снятых обёрток.
    pub(crate) fn new(store: Rc<Store>, nodes: &[Node], allow: bool) -> Option<Rc<Self>> {
        let root = match find(nodes.iter(), "html") {
            Some(html) => Root::Body {
                html: Some(html.node_id),
                body: find(html.children.iter(), "body")?.node_id,
            },
            None => match find(nodes.iter(), "body") {
                Some(body) => Root::Body {
                    html: None,
                    body: body.node_id,
                },
                None => Root::TopLevel,
            },
        };
        Some(Rc::new(Self {
            store,
            allow,
            doc: (nodes.as_ptr() as usize, nodes.len()),
            root,
            seen: Default::default(),
            claimed: Cell::new(false),
            next: Default::default(),
            pending: Default::default(),
            placeholders: Default::default(),
            next_replays: Default::default(),
            pending_replays: Default::default(),
            replay_slots: Default::default(),
            screen: Cell::new(None),
        }))
    }

    /// Элемент этого документа на пути к потоку: запомнить `html` и `body`.
    pub(crate) fn observe(&self, e: &Element, cx: &LayoutCx) {
        let Root::Body { html, body } = self.root else {
            return;
        };
        if cx.contracts.document_root != Some(self.doc) || self.claimed.get() {
            return;
        }
        // Первое появление: потомки с тем же адресом (копии) не в счёт.
        if Some(e.node_id) == html && self.seen.0.get().is_none() {
            let _ = self.seen.0.set(key::box_only(e));
        } else if e.node_id == body && self.seen.1.get().is_none() {
            let _ = self.seen.1.set(key::box_only(e));
        }
    }

    /// Окно перед prepaint дерева: запомнить экран и проверить заместителей.
    /// `false` — кадр нужно собрать заново без них.
    pub(crate) fn validate(&self, window: &mut Window) -> bool {
        let scale = window.scale_factor();
        let mask = window.content_mask().bounds;
        let screen = Bounds {
            origin: scaled(mask.origin, scale),
            size: size(
                ScaledPixels(mask.size.width.as_f32() * scale),
                ScaledPixels(mask.size.height.as_f32() * scale),
            ),
        };
        self.screen.set(Some(screen));
        let offset = window.element_offset() == point(px(0.0), px(0.0));
        if !self.validate_replays(window) {
            return false;
        }
        let placeholders = self.placeholders.borrow();
        placeholders.is_empty()
            || offset
                && placeholders.iter().all(|slot| {
                    slot.layout().is_some_and(|id| {
                        let origin = scaled(window.layout_origin_unrounded(id), scale);
                        slot.record()
                            .ink
                            .is_none_or(|ink| !placed(ink, origin, SNAP).intersects(&screen))
                    })
                })
    }

    /// После краски дерева: записи устоявшихся блоков этого кадра. Ожидающие
    /// записи отдают элемент записи и освобождаются.
    pub(crate) fn finish(&self) {
        let Some(screen) = self.screen.get() else {
            return;
        };
        let mut next = self.next.take();
        let skipped = next.len() as u64;
        let mut recorded = 0;
        for (pending, element) in self.pending.take() {
            if let Some(record) = pending.finish(element, screen) {
                next.insert(record.element.node_id, Rc::new(record));
                recorded += 1;
            }
        }
        // Кадр без записей и пропусков под тем же ключом оставляет прежние:
        // они подойдут следующему кадру, а их сброс стоил бы этому.
        if recorded > 0 || skipped > 0 {
            self.store.records.replace(next);
        }
        self.finish_replays(screen);
        let mut stats = self.store.stats.get();
        stats.recorded += recorded;
        stats.skipped += skipped;
        self.store.stats.set(stats);
    }

    /// Кадр собран заново без заместителей (`validate`).
    pub(crate) fn note_rebuild(&self) {
        let mut stats = self.store.stats.get();
        stats.rebuilt_frames += 1;
        self.store.stats.set(stats);
    }
}

/// Добавить в конец списка в ячейке, не держа её заём.
fn push<T>(cell: &RefCell<Vec<T>>, value: T) {
    let mut all = cell.take();
    all.push(value);
    cell.replace(all);
}
