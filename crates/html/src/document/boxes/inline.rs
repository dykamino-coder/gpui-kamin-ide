//! Логический вход IFC: UTF-8 и границы коробок без геометрии и элементов GPUI.
//!
//! Каждый текст хранится только у ближайшего хозяина потока. Атомы с собственным
//! потоком представлены ключом листа; ширина строки не входит в этот вход.

use crate::dom::{BoxId, Element};
use crate::layout::box_kind;
use crate::style::computed::{Display, RubyRole};
use std::rc::Rc;
use std::string::String;

#[cfg(test)]
mod leaves;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    Text { start: usize, end: usize },
    Open,
    Close,
    HardBreak,
    Opportunity,
    Atom,
    RubyAnnotation,
    Float,
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
    closes: bool,
    owner: Option<usize>,
    content: Option<InlineContent>,
}

struct Class {
    kind: Option<ItemKind>,
    inline: bool,
    contents: bool,
    suppressed: bool,
}

#[derive(Default)]
pub(super) struct Collection {
    frames: Vec<Frame>,
    pub(super) finished: Vec<(BoxId, Option<Rc<InlineContent>>)>,
}

impl Collection {
    pub(super) fn root(&mut self, id: BoxId) {
        self.frames.push(Frame {
            id,
            closes: false,
            owner: Some(0),
            content: Some(InlineContent::default()),
        });
    }

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
        let Class {
            kind,
            inline,
            contents,
            suppressed,
        } = self.classify(element);
        if let Some(kind) = kind {
            self.item(id, kind);
        }
        let inherited = if inline {
            self.frames.last().and_then(|frame| frame.owner)
        } else {
            None
        };
        let owner = inherited.unwrap_or(self.frames.len());
        self.frames.push(Frame {
            id,
            closes: inline && !contents && !suppressed,
            owner: (!suppressed).then_some(owner),
            content: (!suppressed && inherited.is_none()).then(InlineContent::default),
        });
    }

    /// Элемент отдаёт свой текст в IFC предка (строчная коробка, contents).
    pub(super) fn feeds_owner(&self, element: &Element) -> bool {
        let class = self.classify(element);
        class.inline
            && self
                .frames
                .last()
                .is_some_and(|frame| frame.owner.is_some())
    }

    /// Неизменное поддерево: предок получает только элемент строки коробки.
    pub(super) fn skip(&mut self, id: BoxId, element: &Element) {
        if let Some(kind) = self.classify(element).kind {
            self.item(id, kind);
        }
    }

    fn classify(&self, element: &Element) -> Class {
        let style = element.style.used();
        let contents = style.reset_box_layout.display == Some(Display::Contents);
        let special = matches!(element.tag.as_str(), "br" | "wbr");
        let suppressed = self
            .frames
            .last()
            .is_some_and(|frame| frame.owner.is_none())
            || (!contents && (special || box_kind::replaced_tag(element)));
        let out_of_flow = !contents && box_kind::out_of_flow(&style);
        let ruby = crate::text::ruby::ruby_role(element);
        let annotation = matches!(ruby, Some(RubyRole::Text | RubyRole::TextContainer));
        let inline = contents
            || (!out_of_flow
                && box_kind::real_inline(element)
                && !special
                && ruby != Some(RubyRole::Container)
                && !annotation);
        let kind = if contents {
            None
        } else if crate::style::flow::absolutely_positioned(style.reset_positioned.position) {
            Some(ItemKind::OutOfFlow)
        } else if out_of_flow {
            Some(ItemKind::Float)
        } else if element.tag == "br" {
            Some(ItemKind::HardBreak)
        } else if element.tag == "wbr" {
            Some(ItemKind::Opportunity)
        } else if annotation {
            Some(ItemKind::RubyAnnotation)
        } else if inline {
            Some(ItemKind::Open)
        } else if ruby == Some(RubyRole::Container) || box_kind::inline_level_box(element) {
            Some(ItemKind::Atom)
        } else {
            Some(ItemKind::Block)
        };
        Class {
            kind,
            inline,
            contents,
            suppressed,
        }
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
                kind: ItemKind::Text {
                    start,
                    end: content.text.len(),
                },
            });
        }
    }

    pub(super) fn leave(&mut self) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        if frame.closes {
            self.item(frame.id, ItemKind::Close);
        }
        let content = frame.content.filter(|content| !content.items.is_empty());
        self.finished.push((frame.id, content.map(Rc::new)));
    }
}
