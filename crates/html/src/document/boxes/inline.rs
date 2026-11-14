//! Логический вход IFC: UTF-8 и границы коробок без геометрии и элементов GPUI.
//!
//! Каждый текст хранится только у ближайшего хозяина потока. Атомы с собственным
//! потоком представлены ключом листа; ширина строки не входит в этот вход.

use crate::dom::{BoxId, Element};
use crate::layout::box_kind;
use std::ops::Range;
use std::rc::Rc;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    Text(Range<usize>),
    Open,
    Close,
    HardBreak,
    Atom,
    OutOfFlow,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Item {
    pub(crate) box_id: BoxId,
    pub(crate) kind: ItemKind,
}

/// Исходные логические прогоны. Нормализация CSS Text и шейпинг — следующие
/// стадии: диапазоны здесь относятся к этому буферу, а не к исходному DOM.
#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct InlineContent {
    pub(crate) text: String,
    pub(crate) items: Vec<Item>,
}

struct Frame {
    id: BoxId,
    inline: bool,
    owner: Option<usize>,
    content: Option<InlineContent>,
}

#[derive(Default)]
pub(super) struct Collection {
    frames: Vec<Frame>,
    pub(super) finished: Vec<(BoxId, Option<Rc<InlineContent>>)>,
}

impl Collection {
    fn owner(&mut self) -> Option<&mut InlineContent> {
        let index = self.frames.last()?.owner?;
        self.frames[index].content.as_mut()
    }

    fn item(&mut self, box_id: BoxId, kind: ItemKind) {
        if let Some(content) = self.owner() {
            content.items.push(Item { box_id, kind });
        }
    }

    pub(super) fn enter(&mut self, id: BoxId, element: &Element) {
        let out_of_flow = box_kind::out_of_flow(&element.style.used());
        let inline = !out_of_flow && box_kind::real_inline(element) && element.tag != "br";
        let kind = if out_of_flow {
            ItemKind::OutOfFlow
        } else if element.tag == "br" {
            ItemKind::HardBreak
        } else if inline {
            ItemKind::Open
        } else if box_kind::inline_level_box(element) || box_kind::replaced_tag(element) {
            ItemKind::Atom
        } else {
            ItemKind::Block
        };
        self.item(id, kind);
        let owner = if inline {
            self.frames.last().and_then(|frame| frame.owner)
        } else {
            Some(self.frames.len())
        };
        self.frames.push(Frame {
            id,
            inline,
            owner,
            content: (!inline).then(InlineContent::default),
        });
    }

    pub(super) fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let Some(box_id) = self.frames.last().map(|frame| frame.id) else {
            return;
        };
        if let Some(content) = self.owner() {
            let start = content.text.len();
            content.text.push_str(text);
            content.items.push(Item {
                box_id,
                kind: ItemKind::Text(start..content.text.len()),
            });
        }
    }

    pub(super) fn leave(&mut self) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        if frame.inline {
            self.item(frame.id, ItemKind::Close);
        }
        let content = frame.content.filter(|content| !content.items.is_empty());
        self.finished.push((frame.id, content.map(Rc::new)));
    }
}
