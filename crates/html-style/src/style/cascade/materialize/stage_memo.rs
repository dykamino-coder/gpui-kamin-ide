//! Memory of the box stage on a node's style slot. The box stage (`inherit::box_stage`) is a
//! function of its inputs: the facts of its parents (`inherit::ParentFacts`; the element-tree
//! parent too for a moved box), its own style stages, the document's font generation, color
//! scheme and root metrics, and the viewport size, the last only if the values read it
//! (`values::viewport_reads`). A record lives with the node's style (`dom::SharedStyle`) and
//! holds while all inputs are equal: a viewport width change recomputes only the boxes that
//! read the viewport and the boxes under a changed parent; a style or DOM edit gives new style
//! stages and a miss on the next query, so a synchronous geometry query sees the new style
//! without a frame.
//!
//! The best engines work the same way: Blink keeps `ComputedStyle` on the element and
//! recomputes it by `StyleRecalcChange`, Stylo keeps it in element data by restyle hints;
//! shared results are reused by comparing inputs (`MatchedPropertiesCache`, the sharing
//! cache), not by element identity. Frame registry keys (`self_node` -> `cb_node`) are not
//! part of a record: a hit takes them from this frame's parent (`inherit::reidentify`).

use crate::style::cascade::inherit::ParentFacts;
use crate::style::computed::{BoxStyle, Computed, ElementStyle};
use crate::values::ValueCx;
use std::cell::RefCell;
use std::rc::Rc;

/// Records per style: painted and unpainted stages, a moved box, copies of the element.
const ENTRIES: usize = 4;

mod key;
pub use key::OwnKey;
use key::{DocInputs, Viewport, doc_inputs, same_element, viewport};
pub(crate) use key::{Identity, Miss, identity};

/// The inputs of one stage request.
pub(crate) struct Query<'a> {
    pub paint: bool,
    /// Layout moved the box under another box than its element parent's.
    pub moved: bool,
    /// What the stage reads of the parents (`inherit::ParentFacts`).
    pub parent: &'a ParentFacts<'a>,
    /// The element stages of the element parent, which the element's computed variant
    /// follows (`ElementStyle::variant_for`).
    pub element_parent: Option<&'a ElementStyle>,
    /// The layout parent's computed style: an anonymous box computes its own from it.
    pub anonymous: Option<&'a Rc<Computed>>,
    pub own: OwnKey,
    pub vcx: ValueCx<'a>,
}

struct Entry {
    paint: bool,
    moved: bool,
    parent: Rc<ParentFacts<'static>>,
    element_parent: Option<ElementStyle>,
    anonymous: Option<Rc<Computed>>,
    own: OwnKey,
    doc: DocInputs,
    viewport: Option<Viewport>,
    out: BoxStyle,
}

impl Entry {
    fn check(&self, q: &Query<'_>, doc: DocInputs) -> Result<(), Miss> {
        if !self.own.same_stages(&q.own) {
            return Err(Miss::Own);
        }
        if self.own.container != q.own.container {
            return Err(Miss::Container);
        }
        if self.doc.fonts != doc.fonts {
            return Err(Miss::Fonts);
        }
        if self.doc != doc {
            return Err(Miss::Document);
        }
        let anonymous = match (&self.anonymous, q.anonymous) {
            (Some(a), Some(b)) => BoxStyle::same_computed_values(a, b),
            (a, b) => a.is_none() && b.is_none(),
        };
        if *self.parent != *q.parent
            || !same_element(self.element_parent.as_ref(), q.element_parent)
            || !anonymous
        {
            return Err(Miss::Parent);
        }
        if self.viewport.is_some_and(|v| v != viewport(q.vcx)) {
            return Err(Miss::Viewport);
        }
        Ok(())
    }
}
/// The remembered box stages of one style, shared by the copies of its element: the table
/// exists from the start, so a copy made before the first stage (render works on copies of
/// the document's elements) still reads and fills the document's records.
#[derive(Clone, Default)]
pub struct StageMemo(Rc<RefCell<Vec<Entry>>>);

impl std::fmt::Debug for StageMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.0.borrow().len();
        write!(f, "StageMemo({n})")
    }
}

impl StageMemo {
    /// The remembered stage for `q`, or why there is none.
    pub(crate) fn find(&self, q: &Query<'_>) -> Result<BoxStyle, Miss> {
        let entries = &self.0;
        let doc = doc_inputs(q.vcx);
        let mut reason = Miss::First;
        let entries = entries.borrow();
        let same_kind = |e: &&Entry| e.paint == q.paint && e.moved == q.moved;
        for entry in entries.iter().filter(same_kind) {
            match entry.check(q, doc) {
                Ok(()) => {
                    // The values read the viewport for an enclosing computation too.
                    if entry.viewport.is_some() {
                        crate::values::viewport_reads::note(q.vcx);
                    }
                    return Ok(entry.out.clone());
                }
                // A record of this copy of the element names the input that changed.
                Err(miss) if miss != Miss::Own => return Err(miss),
                Err(miss) => reason = miss,
            }
        }
        Err(reason)
    }

    /// Remember `out` computed for `q`; `read_viewport` — its values read the viewport size.
    pub(crate) fn store(&self, q: Query<'_>, read_viewport: bool, out: &BoxStyle) {
        let entry = Entry {
            paint: q.paint,
            moved: q.moved,
            parent: shared_parent(q.vcx, q.parent),
            element_parent: q.element_parent.cloned(),
            anonymous: q.anonymous.cloned(),
            doc: doc_inputs(q.vcx),
            viewport: read_viewport.then(|| viewport(q.vcx)),
            own: q.own,
            out: out.clone(),
        };
        let mut entries = self.0.borrow_mut();
        let same = entries.iter().position(|e| {
            e.paint == entry.paint && e.moved == entry.moved && e.own.same_stages(&entry.own)
        });
        match same {
            Some(i) => entries[i] = entry,
            None => {
                if entries.len() == ENTRIES {
                    entries.remove(0);
                }
                // Most styles keep one record: grow by one, not by the default four.
                entries.reserve_exact(1);
                entries.push(entry);
            }
        }
    }

    /// The records of the style, without the values they share (`heap-report`).
    #[cfg(feature = "heap-report")]
    pub(crate) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        let rc = &self.0;
        ops.at("style/box-stage", |ops| {
            let block = ops.shallow(Rc::as_ptr(rc));
            let entries = rc.borrow();
            if block == 0 || entries.capacity() == 0 {
                return block;
            }
            let parents: usize = (entries.iter())
                .map(|e| ops.shallow(Rc::as_ptr(&e.parent)))
                .sum();
            block + ops.shallow(entries.as_ptr()) + parents
        })
    }
}

/// The parent facts as a key, shared with the siblings keyed by equal ones: children are
/// styled one after another under one parent, so one copy serves a run of siblings.
fn shared_parent(vcx: ValueCx<'_>, parent: &ParentFacts<'_>) -> Rc<ParentFacts<'static>> {
    let mut last = vcx.doc.traversal.stage_parent.borrow_mut();
    if let Some(shared) = &*last
        && **shared == *parent
    {
        return shared.clone();
    }
    let key = Rc::new(parent.clone().into_owned());
    *last = Some(key.clone());
    key
}
/// The size of one record.
#[cfg(test)]
pub(crate) const fn entry_bytes() -> usize {
    std::mem::size_of::<Entry>()
}

#[cfg(test)]
#[path = "stage_memo_tests.rs"]
mod tests;
