//! The logical IFC input: UTF-8 and box boundaries, without geometry or GPUI elements.
//!
//! Every text is kept only by the nearest owner of its flow. Atoms with a flow
//! of their own are represented by a leaf key; the line width is not part of this input.
//!
//! The flows a publication changed share one buffer of text and one of items
//! (`Chunk`); a flow holds its ranges in it, so a document of thousands of
//! paragraphs keeps two allocations instead of three per flow. Blink keeps one
//! text buffer per inline formatting context (`InlineItemsData`); here one
//! buffer serves every flow of a publication. A later publication (a DOM edit,
//! also from script) puts only the flows it changed into a new chunk; an old
//! chunk lives while one of its flows does, so the retained text is at most the
//! live text plus the text of the first publication.

use crate::dom::{BoxId, Element};
use crate::layout::box_kind;
use crate::style::computed::{Display, RubyRole};
use std::string::String;

mod content;
#[cfg(feature = "heap-report")]
pub(crate) use content::Chunk;
pub(super) use content::len as offset;
use content::{Closed, append, len};
pub(crate) use content::{InlineContent, Item, ItemKind};

#[cfg(test)]
mod leaves;
#[cfg(test)]
mod tests;

/// Text and items being collected for one open flow.
#[derive(Default)]
struct Scratch {
    text: String,
    items: Vec<Item>,
}

struct Frame {
    id: BoxId,
    closes: bool,
    owner: Option<usize>,
    content: Option<Scratch>,
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
    /// Buffers of closed flows: the next flow writes into them.
    spare: Vec<Scratch>,
    /// The closed flows of this publication, one after another.
    text: String,
    items: Vec<Item>,
    finished: Vec<Closed>,
}

impl Collection {
    pub(super) fn root(&mut self, id: BoxId) {
        self.frames.push(Frame {
            id,
            closes: false,
            owner: Some(0),
            content: Some(Scratch::default()),
        });
    }

    fn owner(&mut self) -> Option<&mut Scratch> {
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
            content: (!suppressed && inherited.is_none())
                .then(|| self.spare.pop().unwrap_or_default()),
        });
    }

    /// The element gives its text to the IFC of an ancestor (an inline box, contents).
    pub(super) fn feeds_owner(&self, element: &Element) -> bool {
        let class = self.classify(element);
        class.inline
            && self
                .frames
                .last()
                .is_some_and(|frame| frame.owner.is_some())
    }

    /// An unchanged subtree: the ancestor gets only the line item of the box.
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
            let start = len(content.text.len());
            content.text.push_str(text);
            content.items.push(Item {
                box_id,
                kind: ItemKind::Text {
                    start,
                    end: len(content.text.len()),
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
        let ranges = frame.content.and_then(|mut scratch| {
            let ranges = (!scratch.items.is_empty()).then(|| {
                let text = append(&mut self.text, &scratch.text);
                let start = len(self.items.len());
                self.items.append(&mut scratch.items);
                (text, (start, len(self.items.len())))
            });
            scratch.text.clear();
            scratch.items.clear();
            self.spare.push(scratch);
            ranges
        });
        self.finished.push((frame.id, ranges));
    }

    /// The closed flows with their content (`content::seal`).
    pub(super) fn seal<'a>(
        self,
        old: impl Fn(BoxId) -> Option<&'a InlineContent>,
    ) -> Vec<(BoxId, Option<InlineContent>)> {
        content::seal(&self.text, &self.items, &self.finished, old)
    }
}
