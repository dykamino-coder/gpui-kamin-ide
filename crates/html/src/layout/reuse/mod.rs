//! Пропуск устоявшихся блоков корневого потока документа вне экрана.
//!
//! Блочный ребёнок `body`, чьё поддерево после проходов подготовки равно по
//! значениям записанному (`dom::same_tree`) и чей поток имеет тот же ключ
//! (`FlowKey`: окружение `html`/`body`, ограничения коробки, опции хоста,
//! поколение шрифтовых входов), раскладывается так же, как в кадре записи
//! (Blink `LayoutResult` cache, Taffy `Cache`: результат по ключу
//! пространства ограничений). Такой ребёнок не собирается: на его месте
//! точный лист раскладки (`gpui::LayoutSnapshot`) того же размера и полей.
//! Пропуск допустим, только если ни один его примитив не попал бы на экран:
//! записанные чернила (`gpui::InkSink`, с отсечёнными) на новом месте
//! проверяются до prepaint кадра, и любой промах собирает кадр заново без
//! заместителей (`FlowReuse::validate`). Записываются только инертные
//! поддеревья: обычный поток без позиционирования, флоатов и эффектов, без
//! hitbox, состояния элементов, ввода и отложенных отрисовок GPUI, и сборка
//! которых меняет лишь счётчики ключей краски и якорей.

mod flow;
mod key;
mod placeholder;
mod plain;
mod recorder;
#[cfg(test)]
mod tests;

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use gpui::{Bounds, LayoutSnapshot, Pixels, Point, ScaledPixels, Window, point, px, size};
pub(crate) use key::{Counters, FlowKey};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    records: RefCell<HashMap<u64, Rc<Record>>>,
    /// Наблюдение для тестов и стенда: записано, пропущено, кадров заново.
    pub(crate) stats: Cell<ReuseStats>,
}

/// Счётчики наблюдения повтора блоков.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReuseStats {
    pub recorded: u64,
    pub skipped: u64,
    pub rebuilt_frames: u64,
}

/// Запись блока: вход сборки и то, что нужно заместителю и проверке экрана.
pub(crate) struct Record {
    element: Element,
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    snapshot: LayoutSnapshot,
    /// Чернила относительно точного начала коробки; `None` — ничего не рисовал.
    ink: Option<Bounds<ScaledPixels>>,
    /// Точное начало коробки и экран кадра записи.
    origin: Point<ScaledPixels>,
    screen: Bounds<ScaledPixels>,
}

fn scaled(p: Point<Pixels>, scale: f32) -> Point<ScaledPixels> {
    point(
        ScaledPixels(p.x.as_f32() * scale),
        ScaledPixels(p.y.as_f32() * scale),
    )
}

/// Чернила записи, поставленные в начало `origin` и расширенные на `margin`.
fn placed(
    ink: Bounds<ScaledPixels>,
    origin: Point<ScaledPixels>,
    margin: f32,
) -> Bounds<ScaledPixels> {
    Bounds {
        origin: point(
            ScaledPixels(ink.origin.x.0 + origin.x.0 - margin),
            ScaledPixels(ink.origin.y.0 + origin.y.0 - margin),
        ),
        size: size(
            ScaledPixels(ink.size.width.0 + 2.0 * margin),
            ScaledPixels(ink.size.height.0 + 2.0 * margin),
        ),
    }
}

/// Запас проверки экрана на округление краёв, в точках устройства.
const SNAP: f32 = 2.0;

impl Record {
    /// Блок был далеко от экрана: дальше экрана по каждой оси. Сдвиг кадра
    /// обычно меньше, а точный ответ даёт `FlowReuse::validate`.
    fn far(&self) -> bool {
        let Some(ink) = self.ink else { return true };
        let s = self.screen.size;
        let margin = s.width.0.max(s.height.0);
        !placed(ink, self.origin, margin).intersects(&self.screen)
    }
}

/// Повтор блоков одной сборки документа.
pub(crate) struct FlowReuse {
    store: Rc<Store>,
    /// Заместители разрешены; иначе только запись.
    allow: bool,
    root: (usize, usize),
    html: u64,
    body: u64,
    seen: (OnceCell<Element>, OnceCell<Element>),
    claimed: Cell<bool>,
    next: RefCell<HashMap<u64, Rc<Record>>>,
    pending: RefCell<Vec<Rc<recorder::Pending>>>,
    placeholders: RefCell<Vec<Rc<placeholder::Slot>>>,
    screen: Cell<Option<Bounds<ScaledPixels>>>,
}

impl FlowReuse {
    /// Повтор для сборки дерева `nodes`: корень `html` и его `body`.
    pub(crate) fn new(store: Rc<Store>, nodes: &[Node], allow: bool) -> Option<Rc<Self>> {
        let html = nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "html" => Some(e),
            _ => None,
        })?;
        let body = html.children.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "body" => Some(e.node_id),
            _ => None,
        })?;
        Some(Rc::new(Self {
            store,
            allow,
            root: (nodes.as_ptr() as usize, nodes.len()),
            html: html.node_id,
            body,
            seen: Default::default(),
            claimed: Cell::new(false),
            next: Default::default(),
            pending: Default::default(),
            placeholders: Default::default(),
            screen: Cell::new(None),
        }))
    }

    /// Элемент этого документа на пути к потоку: запомнить `html` и `body`.
    pub(crate) fn observe(&self, e: &Element, cx: &LayoutCx) {
        if cx.contracts.document_root != Some(self.root) || self.claimed.get() {
            return;
        }
        // Первое появление: потомки с тем же адресом (копии) не в счёт.
        if e.node_id == self.html && self.seen.0.get().is_none() {
            let _ = self.seen.0.set(e.clone());
        } else if e.node_id == self.body && self.seen.1.get().is_none() {
            let _ = self.seen.1.set(e.clone());
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

    /// После краски дерева: записи устоявшихся блоков этого кадра.
    pub(crate) fn finish(&self) {
        let Some(screen) = self.screen.get() else {
            return;
        };
        let mut next = self.next.take();
        let skipped = next.len() as u64;
        let mut recorded = 0;
        for pending in self.pending.borrow().iter() {
            if let Some(record) = pending.finish(screen) {
                next.insert(record.element.node_id, Rc::new(record));
                recorded += 1;
            }
        }
        self.store.records.replace(next);
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
