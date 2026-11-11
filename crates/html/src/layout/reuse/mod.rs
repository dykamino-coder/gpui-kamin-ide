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

mod far;
mod flow;
mod frame;
mod key;
pub(crate) mod lines;
mod narrow;
mod placeholder;
mod plain;
mod record;
mod recorder;
mod replay;
mod source;
mod store;
#[cfg(test)]
mod tests;
mod width;

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use frame::{placed, scaled};
use gpui::{Bounds, LayoutSnapshot, Pixels, Point, ScaledPixels, Window, point, px, size};
pub(crate) use key::{Counters, FlowKey};
pub(crate) use placeholder::Replay;
pub(crate) use record::Record;
use source::{Shells, Source};
use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
pub(crate) use store::Store;

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
    /// The flow width changes in this build (`narrow`).
    resized: Cell<bool>,
    /// Wraps of this build's blocks (`narrow`).
    narrowed: narrow::Wraps,
    /// Pending records with the comparison inputs of their blocks.
    pending: RefCell<Vec<(Rc<recorder::Pending>, Source)>>,
    /// Shells the sources of this build share (`source`).
    shells: Shells,
    placeholders: RefCell<Vec<Rc<placeholder::Slot>>>,
    next_replays: RefCell<HashMap<u64, Rc<replay::ReplayRecord>>>,
    pending_replays: RefCell<Vec<(Rc<replay::PendingReplay>, Source)>>,
    replay_slots: Cell<Vec<Rc<replay::ReplaySlot>>>,
    /// Far state of this build (`far`): the document's, or made with the
    /// build's first far record or claim.
    far: Cell<Option<Box<far::FrameFar>>>,
    /// Outside width reads of the far candidate being built
    /// (`Mark::shared_widths`): the flow records them here, no flag per build.
    far_widths: Cell<bool>,
    /// Far recorders of this build, at most that many records
    /// (`FlowReuse::add_far`); `FAR_DONE` once the build is finished.
    far_recorders: Cell<u16>,
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
            next_replays: Default::default(),
            pending_replays: Default::default(),
            replay_slots: Default::default(),
            far: Default::default(),
            far_widths: Cell::new(false),
            far_recorders: Cell::new(0),
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

impl crate::layout::cx::WidthSink for FlowReuse {
    fn note(&self) {
        self.far_widths.set(true);
    }
}

/// Добавить в конец списка в ячейке, не держа её заём.
fn push_cell<T>(cell: &Cell<Vec<T>>, value: T) {
    let mut all = cell.take();
    all.push(value);
    cell.set(all);
}

fn push<T>(cell: &RefCell<Vec<T>>, value: T) {
    let mut all = cell.take();
    all.push(value);
    cell.replace(all);
}
