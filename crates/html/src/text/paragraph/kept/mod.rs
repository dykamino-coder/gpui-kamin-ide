//! The paragraph kept across frames (`layout::reuse::paragraph`).
//!
//! After its paint the element moves whole into the slot of its record: its
//! text, runs, spans and the measured boxes of its atoms (`atom_boxes`, the
//! advances in `letter_spans`), none of which depends on the available width
//! while every atom has a fixed inline size. The atoms themselves are kept as
//! the keys of their nodes (`KeptAtoms`), not as elements: a later frame
//! breaks the lines again and builds the element of an atom only where it
//! meets the content mask (`LazyBuild`), as Blink paints only the fragments
//! that meet its cull rect. The boxes of the others are offered as ink
//! (`Window::offer_ink`) to the records that measure it.

mod lazy;
mod paint;
mod prepaint;
#[cfg(test)]
mod tests;

pub(crate) use lazy::{AtomFn, AtomSource, LazyBuild};

use super::*;
use gpui::{Bounds, Pixels};
use std::cell::Cell;
use std::rc::Rc;

/// Where a recorded paragraph goes after its paint, and where the next frame
/// takes it from.
#[derive(Default)]
pub(crate) struct KeptSlot(Cell<Option<Box<Paragraph>>>);

impl KeptSlot {
    pub(crate) fn take(&self) -> Option<Box<Paragraph>> {
        self.0.take()
    }

    pub(crate) fn is_filled(&self) -> bool {
        let kept = self.0.take();
        let filled = kept.is_some();
        self.0.set(kept);
        filled
    }
}

/// The atoms of a kept paragraph in the order of their spacers: the packed
/// node key of each (`Element::node_id`) and its path below the paragraph's
/// nodes, the child indices level by level (`ends[k]` closes atom `k`'s).
/// A path is a cache of the key: the node found there must have the key, so
/// a moved node is never taken for another.
#[derive(Default)]
pub(crate) struct KeptAtoms {
    pub(crate) keys: Vec<u64>,
    pub(crate) path: Vec<u32>,
    pub(crate) ends: Vec<u32>,
}

impl KeptAtoms {
    pub(crate) fn path_of(&self, k: usize) -> &[u32] {
        let start = k.checked_sub(1).map_or(0, |p| self.ends[p] as usize);
        &self.path[start..self.ends[k] as usize]
    }
}

/// Keep state of one paragraph element: boxed, and only on a recorded or
/// resumed paragraph, so a paragraph never kept grows by one pointer.
#[derive(Default)]
pub(crate) struct Kept(Option<Box<State>>);

#[derive(Default)]
struct State {
    /// The slot this paragraph goes to after its paint.
    slot: Option<Rc<KeptSlot>>,
    atoms: Option<Rc<KeptAtoms>>,
    /// Exact layout size of each atom's wrapper when it was laid out.
    sizes: Vec<(f32, f32)>,
    /// The builder of this frame's atom elements; never kept past the frame.
    lazy: Option<LazyBuild>,
    /// Atoms built in this frame: their number and slot.
    built: Vec<(usize, AtomSlot)>,
    /// Union of the boxes of the atoms not built (off the content mask).
    skipped: Option<Bounds<Pixels>>,
    /// An atom built in this frame did not have its kept size.
    broken: bool,
}

impl Kept {
    /// The exact size of atom `k`'s wrapper as `lay_atoms` laid it out.
    pub(crate) fn note_size(&mut self, k: usize, width: f32, height: f32) {
        if let Some(s) = self.0.as_deref_mut().filter(|s| s.slot.is_some()) {
            s.sizes.truncate(k);
            s.sizes.push((width, height));
        }
    }

    fn state(&mut self) -> &mut State {
        self.0.get_or_insert_with(Default::default)
    }

    /// The state of a resumed paragraph in this frame.
    fn resumed(&mut self) -> Option<&mut State> {
        self.0.as_deref_mut().filter(|s| s.lazy.is_some())
    }
}

impl Paragraph {
    /// Record this paragraph, built with all its atoms, into `slot` after its
    /// paint; `atoms` names them by node keys. `false` if it holds anything
    /// that a later frame could not lay out without building it.
    pub(crate) fn keep_into(&mut self, slot: Rc<KeptSlot>, atoms: KeptAtoms) -> bool {
        if atoms.keys.len() != self.atoms.len() || !self.keepable() {
            return false;
        }
        let s = self.kept.state();
        s.slot = Some(slot);
        s.atoms = Some(Rc::new(atoms));
        true
    }

    /// The paragraph of the last frame, taken from `slot`, for this frame:
    /// `lazy` builds the elements of its atoms; it goes back into `slot`.
    pub(crate) fn resume(mut self: Box<Self>, slot: Rc<KeptSlot>, lazy: LazyBuild) -> Self {
        let s = self.kept.state();
        s.slot = Some(slot);
        s.lazy = Some(lazy);
        s.broken = false;
        *self
    }

    /// Whether a later frame may lay this paragraph out without building it:
    /// nothing it holds reads the frame it was built in, or the available
    /// width beyond what the line layout itself reads. The limit of an
    /// orthogonal flow (`ortho_limit`) comes with the container's style, which
    /// the record's key compares.
    fn keepable(&self) -> bool {
        self.overlays.is_empty()
            && self.ruby_base_sink.is_none()
            && self.ruby_layers.is_none()
            && !self.ruby_unit
            && self.fragment.is_none()
            && self.clamp_rows.is_none()
            && self.clamp.is_none()
            && self.fit.is_none()
            && self.inline_measure.is_none()
            && !self.vertical
            && self.vertical_inline.is_none()
            && !self.paint_context.is_rotated()
            && self.spacer_edges.is_empty()
            && self.box_extents.is_empty()
            && self.emph_spans.is_empty()
            && self.flow.0.is_empty()
            && self.flow.1.is_empty()
            && self
                .atoms
                .iter()
                .all(|a| a.fixed_inline && a.extents.is_empty())
            && self.atom_fit.borrow().is_empty()
    }

    /// Every atom has a fixed inline size (`atom_inline_size_fixed`).
    pub(crate) fn atoms_fixed(&self) -> bool {
        self.atoms.iter().all(|a| a.fixed_inline)
    }

    /// The atoms of a kept paragraph by node keys.
    pub(crate) fn kept_atoms(&self) -> Option<Rc<KeptAtoms>> {
        self.kept.0.as_ref().and_then(|s| s.atoms.clone())
    }

    /// A resumed selectable paragraph highlights in this frame's color.
    pub(crate) fn rebind_selection(mut self, color: gpui::Hsla) -> Self {
        if self.id.is_some() {
            self.highlight = color;
        }
        self
    }

    /// Atoms of the paragraph: built ones, or those of a resumed paragraph.
    pub(crate) fn atom_count(&self) -> usize {
        match self.kept.0.as_ref().is_some_and(|s| s.lazy.is_some()) {
            true => self.atom_boxes.len(),
            false => self.atoms.len(),
        }
    }

    /// After the paint: move into the slot, without the frame's elements and
    /// frame state. A paragraph with an atom off its kept size is not kept.
    pub(crate) fn keep_painted(&mut self) {
        let Some(s) = self.kept.0.as_deref_mut() else {
            return;
        };
        let Some(slot) = s.slot.take() else {
            return;
        };
        let atoms = s.atoms.as_ref().map_or(0, |a| a.keys.len());
        if s.broken || s.sizes.len() != atoms || self.atom_boxes.len() != atoms {
            return;
        }
        s.lazy = None;
        s.built = Vec::new();
        s.skipped = None;
        let mut kept = std::mem::replace(self, Paragraph::empty());
        kept.atoms = Vec::new();
        kept.anchor_values = None;
        kept.scroll_owner = None;
        kept.paint_context = Default::default();
        kept.lines = Vec::new();
        slot.0.set(Some(Box::new(kept)));
    }
}
