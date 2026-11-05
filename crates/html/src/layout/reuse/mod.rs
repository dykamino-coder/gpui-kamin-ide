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
mod frame;
mod key;
pub(crate) mod lines;
mod placeholder;
mod plain;
mod record;
mod recorder;
mod source;
#[cfg(test)]
mod tests;
mod width;

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use gpui::{Bounds, LayoutSnapshot, Pixels, Point, ScaledPixels, Window, point, px, size};
pub(crate) use key::{Counters, FlowKey};
pub(crate) use placeholder::Replay;
pub(crate) use record::Record;
use source::{Shells, Source};
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
    /// Ключ кадра до `last`: повторная сборка того же кадра сравнивает с
    /// ним, как первая (`FlowReuse::again`).
    prev: RefCell<Option<Rc<FlowKey>>>,
    /// Blocks whose text wrapped in the last build with a changing flow
    /// width, by node id (`recorder::Narrow`): not recorded in a flow no wider.
    narrow: RefCell<HashMap<u64, Rc<recorder::Narrow>>>,
    /// Блоки, отказанные `FlowReuse::validate`: пересборка кадра их собирает.
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
    fn keep_width_independent(&self, key: &FlowKey) {
        // Записи держат стили: карта меняется целиком, без `&mut` на них.
        let mut records = self.records.take();
        records.retain(|_, r| r.fits_width(key));
        self.records.replace(records);
    }

    /// The flow key changed in more than its width: no record or wrap of
    /// the last key holds.
    fn reset(&self) {
        self.records.take();
        self.narrow.take();
    }

    /// The block `e` wraps in a flow of width `flow` as in the last build
    /// with a changing width (`recorder::Narrow`).
    fn wraps(&self, e: &Element, flow: Option<f32>) -> Option<Rc<recorder::Narrow>> {
        let narrow = self.narrow.borrow().get(&e.node_id).cloned()?;
        narrow.holds(e, flow).then_some(narrow)
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
    /// Из пропущенных — повторены из прошлого кадра (видимые).
    pub replayed: u64,
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
    /// Не первая сборка кадра (после отказа заместителей): ключ потока и
    /// записи первая уже сверила и обновила.
    again: bool,
    doc: (usize, usize),
    root: Root,
    seen: (OnceCell<Element>, OnceCell<Element>),
    claimed: Cell<bool>,
    next: RefCell<HashMap<u64, Rc<Record>>>,
    /// The flow width changes in this build: the wraps of its blocks replace
    /// the store's (`Store::narrow`).
    resized: Cell<bool>,
    /// Wraps of this build's blocks (`recorder::Narrow`).
    narrowed: RefCell<HashMap<u64, Rc<recorder::Narrow>>>,
    /// Pending records with the comparison inputs of their blocks.
    pending: RefCell<Vec<(Rc<recorder::Pending>, Source)>>,
    /// Shells the sources of this build share (`source`).
    shells: Shells,
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
        again: bool,
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
            again,
            doc: (nodes.as_ptr() as usize, nodes.len()),
            root,
            seen: Default::default(),
            claimed: Cell::new(false),
            next: Default::default(),
            resized: Cell::new(false),
            narrowed: Default::default(),
            pending: Default::default(),
            shells: Shells::default(),
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
}

/// Добавить в конец списка в ячейке, не держа её заём.
fn push<T>(cell: &RefCell<Vec<T>>, value: T) {
    let mut all = cell.take();
    all.push(value);
    cell.replace(all);
}
