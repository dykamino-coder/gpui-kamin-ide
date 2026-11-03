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
#[cfg(test)]
mod tests;
mod width;

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use gpui::{
    Bounds, LayoutSnapshot, Pixels, Point, RetainedPaint, RetainedPrepaint, ScaledPixels, Window,
    point, px, size,
};
pub(crate) use key::{Counters, FlowKey};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Записи блоков одного документа между кадрами; переживают `update`.
#[derive(Default)]
pub(crate) struct Store {
    /// Записи под ключом `last`: другой ключ их не принимает, поэтому смена
    /// ключа их сбрасывает, а не держит устаревшие стили.
    records: RefCell<HashMap<u64, Rc<Record>>>,
    /// Ключ потока прошлой сборки: записи делаются, только пока он устоялся,
    /// иначе (ширина меняется каждый кадр) запись не окупается.
    last: RefCell<Option<Rc<FlowKey>>>,
    /// Блоки, чей заместитель не прошёл проверку кадра (`FlowReuse::
    /// validate`): пересборка того же кадра собирает их, а не замещает.
    refused: RefCell<std::collections::HashSet<u64>>,
    /// Номер кадра prepaint документа (`draw`): диапазоны повтора записи
    /// годны только в кадре сразу после своего.
    epoch: Cell<u64>,
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
}

/// Счётчики наблюдения повтора блоков.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReuseStats {
    pub recorded: u64,
    pub skipped: u64,
    pub rebuilt_frames: u64,
    /// Из пропущенных — повторены из прошлого кадра (видимые).
    pub replayed: u64,
}

/// Запись блока: вход сборки и то, что нужно заместителю и проверке экрана.
pub(crate) struct Record {
    /// Вход сборки блока; в куче, чтобы переходить из ожидания без копии.
    element: Box<Element>,
    key: Rc<FlowKey>,
    start: Counters,
    end: Counters,
    snapshot: LayoutSnapshot,
    /// Чернила относительно точного начала коробки; `None` — ничего не рисовал.
    ink: Option<Bounds<ScaledPixels>>,
    /// Раскладка не зависит от ширины потока (`width`): запись годна и при
    /// другой ширине.
    resizable: bool,
    /// Блок был далеко от экрана (`recorder::far`): его пропуск вероятен и
    /// без повтора краски.
    far: bool,
    /// Вывод prepaint и краски блока в последнем кадре, где он рисовался.
    replay: RefCell<Option<Replay>>,
}

/// Вывод блока в кадре `epoch`, который можно повторить в следующем кадре,
/// если блок стоит на том же месте под той же маской (`FlowReuse::validate`):
/// краска GPUI несёт координаты окна и маску. Так повторяется и видимый блок.
#[derive(Clone)]
pub(crate) struct Replay {
    prepaint: RetainedPrepaint,
    paint: RetainedPaint,
    origin: Point<ScaledPixels>,
    mask: Bounds<Pixels>,
    epoch: u64,
}

impl Record {
    /// Повтор записан в последнем кадре prepaint документа до этого.
    fn current(&self, flow: &FlowReuse) -> bool {
        let epoch = flow.store.epoch.get() - u64::from(flow.drawn);
        self.replay
            .borrow()
            .as_ref()
            .is_some_and(|r| r.epoch == epoch)
    }
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
    /// Сборка внутри prepaint: номер кадра уже продвинут.
    drawn: bool,
    doc: (usize, usize),
    root: Root,
    seen: (OnceCell<Element>, OnceCell<Element>),
    claimed: Cell<bool>,
    next: RefCell<HashMap<u64, Rc<Record>>>,
    /// Ожидающие записи с элементами своих блоков.
    pending: RefCell<Vec<(Rc<recorder::Pending>, Box<Element>)>>,
    placeholders: RefCell<Vec<Rc<placeholder::Slot>>>,
    screen: Cell<Option<Bounds<ScaledPixels>>>,
    /// Маска содержимого корня кадра: повтор краски годен только под ней.
    mask: Cell<Option<Bounds<Pixels>>>,
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
    /// `drawn` — prepaint кадра уже начат (`Store::draw`).
    pub(crate) fn new(
        store: Rc<Store>,
        nodes: &[Node],
        allow: bool,
        drawn: bool,
    ) -> Option<Rc<Self>> {
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
            drawn,
            doc: (nodes.as_ptr() as usize, nodes.len()),
            root,
            seen: Default::default(),
            claimed: Cell::new(false),
            next: Default::default(),
            pending: Default::default(),
            placeholders: Default::default(),
            screen: Cell::new(None),
            mask: Cell::new(None),
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
    /// Заместитель годен, если его чернила на новом месте вне экрана либо
    /// его вывод прошлого кадра повторяем на том же месте под той же маской
    /// (тогда он помечается к повтору). `false` — кадр нужно собрать заново:
    /// негодные заместители отказаны (`Store::refused`) и будут собраны, а
    /// годные остаются. Отказанный лист имеет точный размер собранного блока,
    /// поэтому новая раскладка ставит остальные на те же места.
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
        self.mask.set(Some(mask));
        let root = window.element_offset() == point(px(0.0), px(0.0));
        let replayable = root && window.retained_root_environment();
        let epoch = self.store.epoch.get();
        let mut refused = std::collections::HashSet::new();
        for slot in self.placeholders.borrow().iter() {
            let record = slot.record();
            let fits = root
                && slot.layout().is_some_and(|id| {
                    let origin = scaled(window.layout_origin_unrounded(id), scale);
                    let replay = replayable
                        && record.replay.borrow().as_ref().is_some_and(|r| {
                            r.epoch + 1 == epoch
                                && r.origin == origin
                                && r.mask == mask
                                && window.retained_replayable(&r.prepaint, &r.paint)
                        });
                    slot.set_replay(replay.then_some(epoch));
                    replay
                        || record
                            .ink
                            .is_none_or(|ink| !placed(ink, origin, SNAP).intersects(&screen))
                });
            if !fits {
                refused.insert(record.element.node_id);
            }
        }
        let ok = refused.is_empty();
        self.store.refused.borrow_mut().extend(refused);
        ok
    }
    /// После краски дерева: записи устоявшихся блоков этого кадра. Ожидающие
    /// записи отдают элемент записи и освобождаются.
    pub(crate) fn finish(&self) {
        let (Some(screen), Some(mask)) = (self.screen.get(), self.mask.get()) else {
            return;
        };
        self.store.refused.take();
        let mut next = self.next.take();
        let skipped = next.len() as u64;
        let replayed = self
            .placeholders
            .borrow()
            .iter()
            .filter(|slot| slot.replayed())
            .count() as u64;
        let mut recorded = 0;
        let epoch = self.store.epoch.get();
        for (pending, element) in self.pending.take() {
            if let Some(record) = pending.finish(element, screen, mask, epoch) {
                next.insert(record.element.node_id, Rc::new(record));
                recorded += 1;
            }
        }
        // Кадр без записей и пропусков под тем же ключом оставляет прежние:
        // они подойдут следующему кадру, а их сброс стоил бы этому.
        if recorded > 0 || skipped > 0 {
            self.store.records.replace(next);
        }
        let mut stats = self.store.stats.get();
        stats.recorded += recorded;
        stats.skipped += skipped;
        stats.replayed += replayed;
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
