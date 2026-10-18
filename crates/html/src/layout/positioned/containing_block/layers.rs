//! Containing-block layers of one build pass: where out-of-flow boxes wait
//! until the box that contains them takes them as its last children.
//!
//! Our layout under the hood resolves an absolute box against its DIRECT
//! parent; CSS 2.1 §10.1 wants the nearest positioned ancestor (or, for
//! `fixed`, the nearest ancestor that contains fixed boxes; with none, the
//! initial containing block). So the box is built in place and handed to the
//! layer of its containing block. Servo's `PositioningContext` does the same:
//! the containing block opens it, descendants hoist into it, the containing
//! block collects. One instance per build pass (`render`, a nested document)
//! travels in `LayoutCx`, so a nested document never sees an outer layer.

use super::SpotCell;
use super::late::{LatePlace, spot_place};
use crate::hooks::IntoHooked;
use gpui::AnyElement;
use std::cell::RefCell;

type Taken = Vec<(SpotCell, AnyElement)>;

/// Which boxes a nearest-containing-block layer takes.
const ABS: u8 = 1;
const FIX: u8 = 2;

struct CbLayer {
    /// `ABS`, `ABS | FIX` or `FIX` (`open_fixed_only`).
    takes: u8,
    /// `Some(index)` for a strict layer (`cb_open_strict`): it takes only
    /// boxes whose NEAREST containing block is its owner — the CB-ancestor
    /// index of the owner (`Computed::cb_depth` + 1).
    strict: Option<u16>,
    taken: Taken,
}

#[derive(Default)]
pub struct Positioning {
    /// Layers of the nearest containing blocks (positioned boxes).
    cb: RefCell<Vec<CbLayer>>,
    /// Layers of the INITIAL containing block: the document, a scroll strip
    /// block, a nested document. By §10.1 an out-of-flow box without a
    /// positioned ancestor is placed against the viewport, so it becomes one
    /// of the last children of the document.
    icb: RefCell<Vec<Taken>>,
    /// Top paint layers: one per block container being built. A positioned
    /// box at its static position puts itself into the top layer, and the
    /// container appends the layer as its last children.
    late: RefCell<Vec<Taken>>,
    /// Content boxes of flex containers measured in this pass's prepaint
    /// (`absolute_overflow`): an abspos child with safe self-alignment is
    /// placed against its flex parent's content box and its containing block
    /// once both exist. Keyed by the container's node id.
    flex_content: RefCell<std::collections::HashMap<u64, gpui::Bounds<gpui::Pixels>>>,
}

impl Positioning {
    /// Record the content box of flex container `id` (prepaint).
    pub(crate) fn note_flex_content(&self, id: u64, content: gpui::Bounds<gpui::Pixels>) {
        self.flex_content.borrow_mut().insert(id, content);
    }

    /// The content box of flex container `id`, once its prepaint ran.
    pub(crate) fn flex_content(&self, id: u64) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.flex_content.borrow().get(&id).copied()
    }

    /// Open a containing-block layer around the children of a positioned
    /// box; `fixed_cb` — the box also contains `position: fixed` (transform,
    /// `contain: layout|paint`, css-transforms-1 §transform-rendering). Such a
    /// layer takes fixed descendants past intermediate positioned ancestors
    /// (`cb_push_fixed`).
    pub fn cb_open_with(&self, fixed_cb: bool) {
        self.open_cb(ABS | if fixed_cb { FIX } else { 0 }, None);
    }

    /// Open a strict layer around the children of a box whose own style is
    /// `own`: an absolute descendant whose nearest containing block is a box
    /// without a layer below it (a table part, a positioned inline) passes
    /// this layer by and keeps its previous route.
    pub fn cb_open_strict(&self, own: &crate::style::computed::Computed, fixed_cb: bool) {
        self.open_cb(ABS | if fixed_cb { FIX } else { 0 }, Some(own.cb_depth + 1));
    }

    /// Open a layer that takes ONLY `position: fixed`: a multicol box with a
    /// transform (or `contain: layout|paint`) contains fixed descendants
    /// (css-transforms-1 §transform-rendering), but absolute descendants of
    /// the column copies stay in their copies and are cut by the columns
    /// (css-position-3 §abspos-breaking).
    pub fn cb_open_fixed_only(&self) {
        self.open_cb(FIX, None);
    }

    fn open_cb(&self, takes: u8, strict: Option<u16>) {
        self.cb.borrow_mut().push(CbLayer {
            takes,
            strict,
            taken: Vec::new(),
        });
    }

    /// Is a containing-block layer available for this positioned box?
    pub(crate) fn cb_active(&self, fixed: bool) -> bool {
        let mask = if fixed { FIX } else { ABS };
        self.cb.borrow().iter().any(|l| l.takes & mask != 0)
    }

    /// Hand `position: fixed` to the layer of the nearest ancestor that
    /// contains fixed boxes (not the nearest positioned one, CSS 2.1 §10.1
    /// item 3 + css-transforms-1). No layer — the element comes back, paint
    /// it in place.
    pub fn cb_push_fixed(&self, spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
        let mut layers = self.cb.borrow_mut();
        match layers.iter_mut().rev().find(|l| l.takes & FIX != 0) {
            Some(layer) => {
                layer.taken.push((spot, el));
                None
            }
            None => Some(el),
        }
    }

    /// Hand an element to the layer of its nearest containing block, whose
    /// CB-ancestor index is `nearest`. No layer — the element comes back.
    pub fn cb_push(&self, spot: SpotCell, el: AnyElement, nearest: u16) -> Option<AnyElement> {
        let mut layers = self.cb.borrow_mut();
        let layer = layers
            .iter_mut()
            .rev()
            .find(|l| l.takes & ABS != 0 && l.strict.is_none_or(|d| d == nearest));
        match layer {
            Some(layer) => {
                layer.taken.push((spot, el));
                None
            }
            None => Some(el),
        }
    }

    /// Take what the top containing-block layer collected and close it.
    pub fn cb_close(&self) -> Vec<AnyElement> {
        let layer = self.cb.borrow_mut().pop();
        layer
            .map(|l| l.taken)
            .unwrap_or_default()
            .into_iter()
            .map(|(spot, el)| icb_place(spot, el))
            .collect()
    }

    /// Open an initial-containing-block layer: the document, a strip block or
    /// a nested document.
    pub fn icb_open(&self) {
        self.icb.borrow_mut().push(Vec::new());
    }

    /// Take what the top ICB layer collected and close it.
    pub fn icb_close(&self) -> Vec<AnyElement> {
        let layer = self.icb.borrow_mut().pop();
        layer
            .unwrap_or_default()
            .into_iter()
            .map(|(spot, el)| icb_place(spot, el))
            .collect()
    }

    /// Is an ICB layer open?
    ///
    /// A scroll strip builds its subtree in a closure that
    /// `ScrollArea::request_layout` calls AFTER `icb_close()`: candidates can
    /// be hoisted only while the layer still exists.
    pub fn icb_active(&self) -> bool {
        !self.icb.borrow().is_empty()
    }

    /// Hand an element to the top ICB layer. No layer — it comes back.
    pub fn icb_push(&self, spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
        match self.icb.borrow_mut().last_mut() {
            Some(layer) => {
                layer.push((spot, el));
                None
            }
            None => Some(el),
        }
    }

    /// Open the top paint layer for the children of a container.
    pub fn late_open(&self) {
        self.late.borrow_mut().push(Vec::new());
    }

    /// Take what the top paint layer collected and close it.
    pub fn late_close(&self) -> Vec<AnyElement> {
        let layer = self.late.borrow_mut().pop();
        layer
            .unwrap_or_default()
            .into_iter()
            .map(|(spot, el)| spot_place(spot, el))
            .collect()
    }

    /// Does the open top paint layer hold anything? The container's child
    /// loop asks before a positioned sibling: the layer is released before
    /// it, so a static-position abspos paints in tree order (CSS 2.1 app. E,
    /// step 8), not above all positioned siblings after it.
    pub fn late_pending(&self) -> bool {
        self.late
            .borrow()
            .last()
            .is_some_and(|layer| !layer.is_empty())
    }

    /// Hand content to the top paint layer. Without a layer (the element is
    /// built outside a block container) it comes back — paint it in place.
    pub fn late_push(&self, spot: SpotCell, el: AnyElement) -> Option<AnyElement> {
        match self.late.borrow_mut().last_mut() {
            Some(layer) => {
                layer.push((spot, el));
                None
            }
            None => Some(el),
        }
    }
}

/// Заместитель слоя ICB — БЕЗ обёртки, в отличие от `spot_place`.
///
/// Любая коробка вокруг стала бы для раскладки содержащим блоком абсолютного
/// ребёнка (понятия «позиционированный предок» у раскладки нет), и края
/// считались бы от неё, а не от области просмотра — то есть ровно то, ради
/// чего затеян вынос. `LatePlace` своей коробки не заводит: он отдаёт
/// `layout_id` ребёнка.
pub(super) fn icb_place(spot: SpotCell, child: AnyElement) -> AnyElement {
    if spot.get().layout_only {
        return child;
    }
    LatePlace { child, spot }.hooked()
}
