//! Box styles of atomic inlines kept in their box records across frames.
//!
//! An atomic inline (`inline-block`, a replaced element in a line) gets its
//! box style from its own style and the box style of its paragraph
//! (`materialize::box_style`), then resolves viewport units and installs its
//! pseudo-line layers. Every frame of a paragraph rebuilds its atoms, so this
//! work ran again for each atom of each frame, also when nothing it reads
//! changed (a resize of a page of `inline-block` cells). The record of the
//! atom's box keeps the result with its inputs: the own style by identity
//! (`SharedStyle::identical`, then by value), the parent box style by value,
//! the font and image facts of the document, and the viewport only when the
//! computation read it (`viewport_reads`). Equal inputs give the kept style.
//!
//! The atoms of a paragraph share one kept parent, so each compares it by
//! pointer after the first. A computation that wrote a frame handle
//! (`DocumentValues::frame_records`) is not kept: the handle dies with the
//! frame. A later stage continues the kept earlier one, and atoms share
//! equal paint results (`share_paint`), so the kept styles of many like
//! atoms do not each hold their own copy. Blink keeps the same per object: `LayoutObject::Style` survives
//! until `StyleDifference` says otherwise.

use super::BoxTree;
use crate::dom::{Element, SharedStyle};
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use shared::{recent_parent, same_parent, share_paint};
use std::cell::RefCell;
use std::rc::Rc;

mod shared;

/// How far the atom's style goes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// `box_style` of the atom under its parent.
    Merged,
    /// Then viewport units resolved (css-values-4 §6.1.2), as the atom's own
    /// box takes it (`own_box_atom`).
    Resolved,
    /// Then pseudo-line layers installed too (CSS 2 §5.12.1-§5.12.2), as an
    /// `inline-block` takes it.
    PseudoLines,
}

/// One kept computation: its inputs and the styles of its stages.
pub(crate) struct AtomStyle {
    own: SharedStyle,
    parent: Rc<BoxStyle>,
    doc: *const crate::values::DocumentValues,
    fonts: u64,
    images: Rc<crate::resources::css_images::facts::BackgroundImages>,
    stages: [RefCell<Option<Kept>>; 3],
}

struct Kept {
    /// The viewport (`RenderOpts::viewport`, `ValueCx::viewport`) the style
    /// read, if it read one.
    viewport: Option<((f32, f32), (f32, f32))>,
    style: Rc<BoxStyle>,
}

/// The box style of atom `e` under `parent` at `stage`, kept in the record of
/// its box.
pub(crate) fn atom_style(
    e: &Element,
    parent: &BoxStyle,
    opts: &RenderOpts,
    stage: Stage,
) -> Rc<BoxStyle> {
    // Pseudo-line layers read the element's layers, not its style: built anew.
    let keepable =
        stage != Stage::PseudoLines || (e.first_line.is_none() && e.first_letter.is_none());
    let doc = opts.cx.doc();
    let Some(id) = keepable
        .then(|| doc.boxes.borrow().box_of(e.node_id))
        .flatten()
    else {
        return Rc::new(compute(e, parent, opts, stage, None).0);
    };
    let vcx = opts.cx.vcx();
    let viewport = (opts.viewport, vcx.viewport);
    let fonts = vcx.doc.font_generation();
    let images = vcx.doc.background_images.borrow().clone();
    let parent_rc = recent_parent(&doc.boxes, parent);
    let fits = |entry: &AtomStyle| {
        std::ptr::eq(entry.doc, vcx.doc)
            && entry.fonts == fonts
            && Rc::ptr_eq(&entry.images, &images)
            && (Rc::ptr_eq(&entry.parent, &parent_rc) || same_parent(&entry.parent, parent))
    };
    let kept = doc
        .boxes
        .borrow()
        .get(id)
        .and_then(|r| r.atom_style.clone());
    let entry = kept
        .filter(|entry| fits(entry) && (entry.own.identical(&e.style) || entry.own.same(&e.style)))
        .unwrap_or_else(|| {
            Rc::new(AtomStyle {
                own: e.style.clone(),
                parent: parent_rc,
                doc: vcx.doc,
                fonts,
                images,
                stages: Default::default(),
            })
        });
    let staged = Staged {
        e,
        parent,
        opts,
        entry: &entry,
        viewport,
    };
    let (style, _, frame_free) = staged.style(stage, true);
    if frame_free {
        store(&doc.boxes, id, &entry);
    }
    style
}

impl AtomStyle {
    fn slot(&self, stage: Stage) -> &RefCell<Option<Kept>> {
        &self.stages[stage as usize]
    }
}

impl Stage {
    /// The stage this one continues.
    fn before(self) -> Option<Stage> {
        match self {
            Stage::Merged => None,
            Stage::Resolved => Some(Stage::Merged),
            Stage::PseudoLines => Some(Stage::Resolved),
        }
    }
}

/// One atom's stages under the inputs of its entry.
struct Staged<'a> {
    e: &'a Element,
    parent: &'a BoxStyle,
    opts: &'a RenderOpts,
    entry: &'a AtomStyle,
    viewport: ((f32, f32), (f32, f32)),
}

impl Staged<'_> {
    /// The style of `stage`, whether it read the viewport and whether it wrote
    /// no frame handle. A later stage continues the earlier one, kept if a
    /// caller asked for it; only a stage asked for (keep) is kept, so an atom
    /// holds the styles its callers read and no steps between them.
    fn style(&self, stage: Stage, keep: bool) -> (Rc<BoxStyle>, bool, bool) {
        if let Some(kept) = (self.entry.slot(stage).borrow().as_ref())
            .filter(|k| k.viewport.is_none_or(|v| v == self.viewport))
        {
            return (kept.style.clone(), kept.viewport.is_some(), true);
        }
        let from = stage.before().map(|before| self.style(before, false));
        let (mut style, read, frame_free) = compute(
            self.e,
            self.parent,
            self.opts,
            stage,
            from.as_ref().map(|(style, _, _)| &**style),
        );
        share_paint(&self.opts.cx.doc().boxes, &mut style);
        let read = read || from.as_ref().is_some_and(|(_, read, _)| *read);
        let frame_free = frame_free && from.as_ref().is_none_or(|(_, _, free)| *free);
        let style = Rc::new(style);
        if keep && frame_free {
            *self.entry.slot(stage).borrow_mut() = Some(Kept {
                viewport: read.then_some(self.viewport),
                style: style.clone(),
            });
        }
        (style, read, frame_free)
    }
}

/// The style of `stage` (continuing `from`, the style of the stage before,
/// if given), whether it read the viewport, and whether it wrote no frame
/// handle.
fn compute(
    e: &Element,
    parent: &BoxStyle,
    opts: &RenderOpts,
    stage: Stage,
    from: Option<&BoxStyle>,
) -> (BoxStyle, bool, bool) {
    let vcx = opts.cx.vcx();
    let before = vcx.doc.frame_records();
    let (style, read) = crate::values::viewport_reads::track(vcx.doc, || {
        let (mut style, resolve) = match from {
            Some(from) => (from.clone(), stage == Stage::Resolved),
            None => (box_style(parent, &e.style, vcx), stage != Stage::Merged),
        };
        if resolve {
            style.resolve_viewport(opts.viewport, vcx);
        }
        if stage == Stage::PseudoLines {
            crate::layout::box_tree::pseudo_line_layers::install(e, &mut style, vcx);
        }
        style
    });
    (style, read, vcx.doc.frame_records() == before)
}

/// Keep `entry` in the record of box `id`, unless the box tree is borrowed.
fn store(boxes: &RefCell<BoxTree>, id: crate::dom::BoxId, entry: &Rc<AtomStyle>) {
    let Ok(mut boxes) = boxes.try_borrow_mut() else {
        return;
    };
    if let Some(record) = boxes.arena.record_mut(id)
        && !record
            .atom_style
            .as_ref()
            .is_some_and(|kept| Rc::ptr_eq(kept, entry))
    {
        record.atom_style = Some(entry.clone());
    }
}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for AtomStyle {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        let kept: usize = (self.stages.iter())
            .map(|stage| {
                stage.borrow().as_ref().map_or(0, |kept| {
                    let style = &kept.style;
                    ops.shallow(Rc::as_ptr(style))
                        + style.style_rc().heap_size(ops)
                        + (style.paint.as_ref()).map_or(0, |paint| ops.shallow(Rc::as_ptr(paint)))
                })
            })
            .sum();
        kept + self.own.heap_size(ops) + ops.shallow(Rc::as_ptr(&self.parent))
    }
}

#[cfg(test)]
#[path = "atom_styles_tests.rs"]
mod tests;
