//! Слои containing block одного прохода построения: где ждут вынесенные из потока
//! боксы, пока содержащий их бокс не заберёт их последними потомками.
//!
//! Наш layout «под капотом» разрешает absolute-бокс относительно его ПРЯМОГО
//! родителя; CSS 2.1 §10.1 требует ближайшего позиционированного предка (для
//! `fixed` — ближайшего предка, содержащего fixed-боксы; при его отсутствии —
//! начальный containing block). Поэтому бокс строится на месте и передаётся слою
//! своего containing block. Так же делает `PositioningContext` в Servo: containing
//! block открывает его, потомки поднимаются в него, containing block собирает.
//! Один экземпляр на проход построения (`render`, вложенный документ) едет в
//! `LayoutCx`, поэтому вложенный документ никогда не видит внешний слой.

use super::SpotCell;
use super::late::spot_place;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;
use std::cell::RefCell;

mod initial;
pub(super) use initial::icb_place;
mod scope;
pub(crate) use scope::LayerScope;

use super::oof::{OofQueue, Pending};
mod probe_scope;
mod retained;
pub(crate) use retained::Escaped;

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
    taken: OofQueue,
    /// Scroll node whose content the layer's owner lays its boxes out in
    /// (`ScrollContext::node`); `None` until `cb_set_node`.
    node: Option<gpui::ScrollHandle>,
}

#[derive(Default)]
pub struct Positioning {
    /// Layers of the nearest containing blocks (positioned boxes).
    cb: RefCell<Vec<CbLayer>>,
    pending: RefCell<Vec<Pending>>,
    /// Layers of the INITIAL containing block: the document, a scroll strip
    /// block, a nested document. By §10.1 an out-of-flow box without a
    /// positioned ancestor is placed against the viewport, so it becomes one
    /// of the last children of the document.
    icb: RefCell<Vec<OofQueue>>,
    /// For each ICB layer: the scroll handle of the root scrollport whose
    /// padding box is this initial containing block (`html-viewport`), or
    /// `None`. A fixed box of that layer hangs on the viewport, not on the
    /// scrolled content (`viewport_anchor`).
    anchors: RefCell<Vec<Option<gpui::ScrollHandle>>>,
    /// Top paint layers: one per block container being built. A positioned
    /// box at its static position puts itself into the top layer, and the
    /// container appends the layer as its last children.
    late: RefCell<Vec<OofQueue>>,
    /// Content boxes of flex containers measured in this pass's prepaint
    /// (`absolute_overflow`): an abspos child with safe self-alignment is
    /// placed against its flex parent's content box and its containing block
    /// once both exist. Keyed by the container's node id.
    pub(in crate::layout::positioned) flex_content:
        crate::layout::probe::Sink<(u64, gpui::Bounds<gpui::Pixels>)>,
}

impl Positioning {
    /// The content box of flex container `id`, once its prepaint ran.
    pub(crate) fn flex_content(&self, id: u64) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.flex_content
            .borrow()
            .iter()
            .rev()
            .find(|(key, _)| *key == id)
            .map(|(_, rect)| *rect)
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
    pub fn cb_open_strict(&self, own: &BoxStyle, fixed_cb: bool) {
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
            taken: OofQueue::with_pending(
                self.pending.borrow().last().cloned().unwrap_or_default(),
            ),
            node: None,
        });
    }

    /// Record the scroll node of the containing-block layer opened last.
    pub(crate) fn cb_set_node(&self, node: Option<gpui::ScrollHandle>) {
        if let Some(layer) = self.cb.borrow_mut().last_mut() {
            layer.node = node;
        }
    }

    /// The scroll node of the layer a box goes to (`icb_push`, `cb_push_fixed`
    /// or `cb_push` for the box's nearest containing block): the space its layout half is placed in.
    pub(crate) fn layer_node(
        &self,
        to_icb: bool,
        far_fixed: bool,
        inherited: &BoxStyle,
    ) -> Option<gpui::ScrollHandle> {
        if to_icb {
            return self.viewport_anchor();
        }
        let nearest = super::nearest_cb_index(inherited);
        let layers = self.cb.borrow();
        let mut open = layers.iter().rev();
        let layer = if far_fixed {
            open.find(|l| l.takes & FIX != 0)
        } else {
            open.find(|l| l.takes & ABS != 0 && l.strict.is_none_or(|d| d == nearest))
        };
        layer.and_then(|l| l.node.clone())
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
                layer.taken.push(spot, el);
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
                layer.taken.push(spot, el);
                None
            }
            None => Some(el),
        }
    }

    /// Take what the top containing-block layer collected and close it.
    pub fn cb_close(&self) -> Vec<AnyElement> {
        let layer = self.cb.borrow_mut().pop();
        layer.map(|l| l.taken).unwrap_or_default().place(icb_place)
    }

    /// Open the top paint layer for the children of a container.
    pub fn late_open(&self) {
        self.late.borrow_mut().push(OofQueue::with_pending(
            self.pending.borrow().last().cloned().unwrap_or_default(),
        ));
    }

    /// Take what the top paint layer collected and close it.
    pub fn late_close(&self) -> Vec<AnyElement> {
        let layer = self.late.borrow_mut().pop();
        layer.unwrap_or_default().place(spot_place)
    }

    /// Boxes waiting in the open layers: a subtree whose build leaves this
    /// unchanged handed none of its boxes to a layer outside it.
    pub(crate) fn queued(&self) -> usize {
        let cb: usize = self.cb.borrow().iter().map(|l| l.taken.len()).sum();
        let icb: usize = self.icb.borrow().iter().map(OofQueue::len).sum();
        let late: usize = self.late.borrow().iter().map(OofQueue::len).sum();
        cb + icb + late
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
                layer.push(spot, el);
                None
            }
            None => Some(el),
        }
    }
}
