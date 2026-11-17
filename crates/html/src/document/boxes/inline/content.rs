//! The stored inline content of a flow: its text, and its items as a range
//! of a chunk shared by the flows of one publication.
//!
//! The text is the flow's own shared string: a short one is inline, a long
//! one is one shared buffer that the paragraph built from an unchanged flow
//! takes as its own text (`InlineContent::shared_text`). Blink copies the
//! DOM text into `InlineItemsData::text_content` and again into each shape
//! result's string; here the box tree and the paragraph share one copy.

use crate::dom::BoxId;
use gpui::SharedString;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ItemKind {
    /// Byte offsets into the flow's text (`InlineContent::text`).
    Text {
        start: u32,
        end: u32,
    },
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

impl ItemKind {
    /// The byte range of a text item in its flow's text.
    pub(crate) fn text_range(&self) -> Option<std::ops::Range<usize>> {
        match *self {
            Self::Text { start, end } => Some(start as usize..end as usize),
            _ => None,
        }
    }
}

/// Items of the flows of one publication (see the module).
#[derive(Debug, Default)]
pub(crate) struct Chunk {
    items: Box<[Item]>,
}

/// The source logical runs of one flow: its text and a range of shared items.
/// CSS Text normalization and shaping are the next stages: the offsets of text
/// items are relative to the text of this flow, not to the source DOM.
#[derive(Clone, Debug)]
pub(crate) struct InlineContent {
    text: SharedString,
    chunk: Rc<Chunk>,
    items: (u32, u32),
}

impl InlineContent {
    /// A flow in a chunk of its own.
    pub(crate) fn single(text: String, items: Vec<Item>) -> Self {
        let items_len = len(items.len());
        Self {
            text: SharedString::from(text),
            chunk: Rc::new(Chunk {
                items: items.into_boxed_slice(),
            }),
            items: (0, items_len),
        }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The text as a shared string: a clone shares its buffer.
    pub(crate) fn shared_text(&self) -> SharedString {
        self.text.clone()
    }

    pub(crate) fn items(&self) -> &[Item] {
        &self.chunk.items[self.items.0 as usize..self.items.1 as usize]
    }

    /// The same stored flow, not only an equal one.
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.chunk, &other.chunk)
            && self.items == other.items
            && self.text == other.text
    }
}

impl PartialEq for InlineContent {
    fn eq(&self, other: &Self) -> bool {
        self.same(other) || (self.text() == other.text() && self.items() == other.items())
    }
}

impl Eq for InlineContent {}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for InlineContent {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        // A text longer than the inline capacity of `SmolStr` (23 bytes) is
        // one shared block: the `Arc` counters, then the bytes.
        const INLINE: usize = 23;
        const COUNTERS: usize = 2 * size_of::<usize>();
        let text = if self.text.len() > INLINE {
            ops.buffer(
                self.text.as_ptr().wrapping_sub(COUNTERS),
                self.text.len() + COUNTERS,
            )
        } else {
            0
        };
        text + ops.buffer(self.chunk.items.as_ptr(), self.chunk.items.len())
    }
}

/// An offset in an inline buffer (below 4 GiB on any page a browser loads).
pub(crate) fn len(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A closed flow of a publication: its text and item ranges in the
/// publication's buffers.
pub(super) type Closed = (BoxId, Option<((u32, u32), (u32, u32))>);

/// The closed flows with their content: a flow equal to its previous content
/// (`old`) keeps it, the others go into one new chunk.
pub(super) fn seal<'a>(
    all_text: &str,
    all_items: &[Item],
    finished: &[Closed],
    old: impl Fn(BoxId) -> Option<&'a InlineContent>,
) -> Vec<(BoxId, Option<InlineContent>)> {
    enum Kept {
        Old(InlineContent),
        New(SharedString, (u32, u32)),
        None,
    }
    let mut items = Vec::new();
    let kept: Vec<_> = finished
        .iter()
        .map(|&(id, ranges)| {
            let Some((t, i)) = ranges else {
                return (id, Kept::None);
            };
            let t_slice = &all_text[t.0 as usize..t.1 as usize];
            let i_slice = &all_items[i.0 as usize..i.1 as usize];
            if let Some(old) = old(id).filter(|o| o.text() == t_slice && o.items() == i_slice) {
                return (id, Kept::Old(old.clone()));
            }
            let start = len(items.len());
            items.extend_from_slice(i_slice);
            (
                id,
                Kept::New(SharedString::from(t_slice), (start, len(items.len()))),
            )
        })
        .collect();
    let chunk = Rc::new(Chunk {
        items: items.into_boxed_slice(),
    });
    kept.into_iter()
        .map(|(id, kept)| {
            let content = match kept {
                Kept::Old(old) => Some(old),
                Kept::New(text, items) => Some(InlineContent {
                    chunk: chunk.clone(),
                    text,
                    items,
                }),
                Kept::None => None,
            };
            (id, content)
        })
        .collect()
}

/// Append `text` to `buffer`: its range there.
pub(super) fn append(buffer: &mut String, text: &str) -> (u32, u32) {
    let start = len(buffer.len());
    buffer.push_str(text);
    (start, len(buffer.len()))
}
