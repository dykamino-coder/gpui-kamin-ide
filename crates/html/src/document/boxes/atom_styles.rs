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
//! frame. Blink keeps the same per object: `LayoutObject::Style` survives
//! until `StyleDifference` says otherwise.

use super::BoxTree;
use crate::dom::{Element, SharedStyle};
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use std::cell::RefCell;
use std::rc::Rc;

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

/// One kept computation: its inputs and the styles of both stages.
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

thread_local! {
    /// The parent box style of the latest atoms.
    static PARENT: RefCell<Option<Rc<BoxStyle>>> = const { RefCell::new(None) };
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
        return Rc::new(compute(e, parent, opts, stage).0);
    };
    let vcx = opts.cx.vcx();
    let viewport = (opts.viewport, vcx.viewport);
    let fonts = vcx.doc.font_generation();
    let images = vcx.doc.background_images.borrow().clone();
    let parent_rc = recent_parent(parent);
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
        .filter(|entry| fits(entry) && (entry.own.identical(&e.style) || entry.own.same(&e.style)));
    if let Some(entry) = &entry
        && let Some(style) = entry
            .slot(stage)
            .borrow()
            .as_ref()
            .filter(|k| k.viewport.is_none_or(|v| v == viewport))
    {
        let style = style.style.clone();
        store(&doc.boxes, id, entry);
        return style;
    }
    let (style, read, frame_free) = compute(e, parent, opts, stage);
    let style = Rc::new(style);
    if !frame_free {
        return style;
    }
    let entry = entry.unwrap_or_else(|| {
        Rc::new(AtomStyle {
            own: e.style.clone(),
            parent: parent_rc,
            doc: vcx.doc,
            fonts,
            images,
            stages: Default::default(),
        })
    });
    *entry.slot(stage).borrow_mut() = Some(Kept {
        viewport: read.then_some(viewport),
        style: style.clone(),
    });
    store(&doc.boxes, id, &entry);
    style
}

impl AtomStyle {
    fn slot(&self, stage: Stage) -> &RefCell<Option<Kept>> {
        &self.stages[stage as usize]
    }
}

/// The style of `stage`, whether it read the viewport, and whether it wrote
/// no frame handle.
fn compute(
    e: &Element,
    parent: &BoxStyle,
    opts: &RenderOpts,
    stage: Stage,
) -> (BoxStyle, bool, bool) {
    let vcx = opts.cx.vcx();
    let before = vcx.doc.frame_records();
    let (style, read) = crate::values::viewport_reads::track(|| {
        let mut merged = box_style(parent, &e.style, vcx);
        if stage != Stage::Merged {
            merged.resolve_viewport(opts.viewport, vcx);
        }
        if stage == Stage::PseudoLines {
            crate::layout::box_tree::pseudo_line_layers::install(e, &mut merged, vcx);
        }
        merged
    });
    (style, read, vcx.doc.frame_records() == before)
}

/// The kept parent if `parent` equals it, else a new one kept for the next
/// atoms: the atoms of a paragraph compare their parent once.
fn recent_parent(parent: &BoxStyle) -> Rc<BoxStyle> {
    PARENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        match &*recent {
            Some(kept) if same_parent(kept, parent) => kept.clone(),
            _ => {
                let kept = Rc::new(parent.clone());
                *recent = Some(kept.clone());
                kept
            }
        }
    })
}

/// Every value of a parent box style that `box_style` reads.
fn same_parent(a: &BoxStyle, b: &BoxStyle) -> bool {
    let paint = match (&a.paint, &b.paint) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y) || **x == **y,
        _ => false,
    };
    a.same_box_values(b)
        && paint
        && a.self_node == b.self_node
        && a.decor_box == b.decor_box
        && a.cb_node == b.cb_node
        && a.fixed_cb_node == b.fixed_cb_node
        && a.anchor_key == b.anchor_key
        && same_element(a.element_style.as_ref(), b.element_style.as_ref())
}

fn same_element(
    a: Option<&crate::style::computed::ElementStyle>,
    b: Option<&crate::style::computed::ElementStyle>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => {
            Rc::ptr_eq(&x.original, &y.original)
                && Rc::ptr_eq(&x.computed, &y.computed)
                && match (&x.parent, &y.parent) {
                    (None, None) => true,
                    (Some(p), Some(q)) => Rc::ptr_eq(p, q) || same_element(Some(p), Some(q)),
                    _ => false,
                }
        }
        _ => false,
    }
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

#[cfg(test)]
#[path = "atom_styles_tests.rs"]
mod tests;
