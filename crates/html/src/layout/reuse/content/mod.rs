//! Повтор содержимого коробки на любой глубине без сборки его элементов.
//!
//! Содержимое блочного контейнера (`render::blocks::stack`: дети ячейки,
//! блока, элемента списка) собирается из поддерева коробки и из того, что
//! контейнер даёт ему снаружи: стиля контейнера, ограничений, окружения
//! документа и счётчиков сборки. Если ни одна коробка поддерева не менялась
//! с кадра записи (эпоха сборки `Epochs::paint` не новее записи) и вход
//! снаружи тот же, сборка дала бы те же элементы: каждый элемент вывода
//! повторяет свои узлы раскладки, prepaint и краску прошлого кадра
//! (`replay`), а проверка кадра (`FlowReuse::validate_replays`) отвергает
//! повтор, который встал не туда или которому понадобилась новая мера.
//!
//! Чистоту решает дерево коробок за O(1) на контейнер, без сравнения
//! поддерева: правка DOM (в том числе из JS) публикуется записью эпох, и
//! содержимое на пути к ней собирается, а остальное повторяется. Так
//! LayoutNG берёт кэшированный `LayoutResult` чистой коробки, а Blink
//! `PaintController` — подпоследовательность краски чистого поддерева.

mod key;
pub(crate) mod shared;

pub(super) use key::inert_tree;

use super::replay::{PendingReplay, Replay, ReplayRecord, ReplayRecorder, ReplaySlot};
use super::{Counters, FlowReuse, Source};
use crate::dom::{BoxId, Node};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use key::{ContentKey, key, plain};
use std::cell::Cell;
use std::rc::Rc;

/// Запись содержимого: вход, номер публикации и элементы последнего кадра.
pub(crate) struct ContentRecord {
    key: ContentKey,
    /// `BoxTree::clock` сборки записи: годна, пока эпоха сборки коробки не новее.
    at: u32,
    start: Counters,
    end: Counters,
    items: Vec<Rc<ReplayRecord>>,
}

/// Содержимое, собранное в этой сборке с обёртками записи.
pub(super) struct PendingContent {
    owner: BoxId,
    record: ContentRecord,
    items: Vec<Rc<PendingReplay>>,
}

/// Содержимое контейнера `inherited` в этой сборке: ключ для повтора и записи.
pub(crate) struct Content {
    owner: BoxId,
    key: ContentKey,
    at: u32,
    start: Counters,
}

impl FlowReuse {
    /// Содержимое `nodes` контейнера `inherited` под опциями детей `inner`
    /// (`outer` — опции контейнера), если его стоит повторять или записывать.
    pub(crate) fn content(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        outer: &RenderOpts,
        inner: &RenderOpts,
        fresh: bool,
    ) -> Option<Content> {
        // The root flow has its own records (`FlowCx`).
        let root = match self.root {
            super::Root::Body { html, body } => {
                inherited.self_node == body || Some(inherited.self_node) == html
            }
            super::Root::TopLevel => inherited.reset_context.icb,
        };
        if root
            || nodes.is_empty()
            || outer.cx.contracts.document_root != Some(self.doc)
            || !plain(nodes, inner)
        {
            return None;
        }
        let (owner, at) = {
            let boxes = inner.cx.doc().boxes.borrow();
            let owner = boxes.owner_of(nodes)?;
            // A block of the root flow is replayed whole by its records
            // (`FlowCx`), whose recorders hold no content recorder.
            let parent = boxes.get(owner)?.parent;
            let flow = match self.root {
                super::Root::Body { body, .. } => boxes.box_of(body),
                super::Root::TopLevel => Some(crate::dom::BoxId::NONE),
            };
            if flow == Some(parent) || boxes.get(parent).is_none() {
                return None;
            }
            (owner, boxes.clock())
        };
        let cx = &inner.cx;
        Some(Content {
            owner,
            at,
            start: Counters::read(cx),
            key: ContentKey {
                inherited: self.store.inputs.style(inherited),
                ids: key::ids(inherited),
                constraints: cx.constraints.clone(),
                cell_bfc: outer.cx.constraints.cell_bfc,
                fresh,
                len: nodes.len(),
                root: self.store.inputs.root(&inner.root_style()),
                viewport: inner.viewport,
                text: self.store.inputs.text(&inner.text),
                normal_line_height: inner.normal_line_height,
                fonts: cx.doc().values.font_generation(),
            },
        })
    }

    /// Повтор содержимого, если его запись годна (см. модуль).
    pub(crate) fn replay_content(
        &self,
        content: &Content,
        opts: &RenderOpts,
    ) -> Option<Vec<AnyElement>> {
        if !self.allow || self.again || self.store.banned(key(content.owner)) {
            return None;
        }
        let record = self
            .store
            .contents
            .borrow()
            .get(&key(content.owner))
            .cloned();
        let record = record?;
        let clean = {
            let boxes = opts.cx.doc().boxes.borrow();
            let epoch = boxes.build_epoch(content.owner)?;
            // Its width may follow a changed sibling (a table column): the
            // replay would be refused after layout, and the frame built twice.
            epoch <= record.at
                && record.at <= boxes.clock()
                && !boxes.resized_by_siblings(content.owner, record.at)
        };
        if !clean || !record.key.same(&content.key) {
            return None;
        }
        content
            .start
            .advanced(record.start, record.end)
            .write(&opts.cx);
        let out = record
            .items
            .iter()
            .map(|item| {
                let slot = Rc::new(ReplaySlot::new(item.clone()));
                super::push_cell(&self.replay_slots, slot.clone());
                Replay::new(slot).hooked()
            })
            .collect();
        let mut next = self.next_contents.take();
        next.insert(key(content.owner), record);
        self.next_contents.replace(next);
        Some(out)
    }

    /// Обернуть собранное содержимое записью (см. модуль).
    pub(crate) fn record_content(
        &self,
        content: Content,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        let end = Counters::read(&opts.cx);
        let mut items = Vec::with_capacity(out.len());
        for child in std::mem::take(out) {
            let pending = Rc::new(PendingReplay::new(
                None,
                (content.start, end),
                true,
                Rc::new(Cell::new(false)),
                false,
                &self.ink,
                None,
            ));
            items.push(pending.clone());
            out.push(ReplayRecorder::new(child, pending).hooked());
        }
        let mut pending = self.pending_contents.take();
        pending.push(PendingContent {
            owner: content.owner,
            record: ContentRecord {
                key: content.key,
                at: content.at,
                start: content.start,
                end,
                items: Vec::new(),
            },
            items,
        });
        self.pending_contents.replace(pending);
    }

    /// Записи содержимого этого кадра: повторённые и завершённые.
    pub(super) fn finish_contents(&self) {
        let mut next = self.next_contents.take();
        for pending in self.pending_contents.take() {
            let PendingContent {
                owner,
                mut record,
                items,
            } = pending;
            let finished: Option<Vec<_>> = items
                .iter()
                .map(|item| item.finish(Source::bare(key(owner)), None).map(Rc::new))
                .collect();
            if let Some(items) = finished {
                record.items = items;
                next.insert(key(owner), Rc::new(record));
            }
        }
        self.store.contents.replace(next);
    }
}
