//! The measure of a far block: built alone in a private arena and layout
//! engine at the width its leaf is given, then dropped (`leaf::Measured`).

use super::Slot;
use crate::dom::Node;
use crate::hooks::{IntoHooked, Wrap};
use crate::layout::options::RenderOpts;
use crate::layout::reuse::Counters;
use crate::style::computed::BoxStyle;
use gpui::{
    AnyElement, App, AvailableSpace, IntoElement, LayoutId, MeasuredContent, ParentElement, Pixels,
    Size, Styled, Window, div, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

/// What the measure of a far block builds it from.
pub(super) struct Source {
    /// The block without its inline margins: its leaf has them.
    pub(super) node: Node,
    pub(super) inherited: BoxStyle,
    /// The block's options without the document's records: the private build
    /// registers nothing with the frame.
    pub(super) opts: RenderOpts,
}

/// Records the layout node of the block the measure builds.
struct Probe {
    child: AnyElement,
    id: Rc<Cell<Option<LayoutId>>>,
}

impl Wrap for Probe {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.id.set(Some(id));
        id
    }
}

/// The far block of `source` laid out alone at the width the parent gives
/// its leaf; its elements, nodes and shaped lines go before this returns.
fn measure(
    source: &Source,
    slot: &Slot,
    known: Size<Option<Pixels>>,
    available: Size<AvailableSpace>,
    window: &mut Window,
    cx: &mut App,
) -> MeasuredContent {
    let width = known
        .width
        .map_or(available.width, AvailableSpace::Definite);
    // Layout asks a leaf for the same width more than once (intrinsic
    // sizes, then the final pass): the block is built once per width.
    let cached = slot.sizes.borrow().iter().find(|s| s.0 == width).copied();
    if let Some((_, laid, snapshot)) = cached {
        slot.measured.set(snapshot.or(slot.measured.get()));
        return content(laid, snapshot);
    }
    if slot.failed.get() {
        return MeasuredContent::new(Size::default());
    }
    let counters = Counters::read(&source.opts.cx);
    let laid_out = gpui::with_temporary_element_arena(|| {
        window.with_nested_layout(|window| {
            window.with_line_layouts_released(|window| {
                let opts = &source.opts;
                let build = &opts.cx.contracts.build;
                let mut kids = build.prepared_block(&source.node, &source.inherited, opts);
                if kids.len() != 1 {
                    return None;
                }
                let id = Rc::new(Cell::new(None));
                gpui::note_layoutless_element();
                let probe = Probe {
                    child: kids.pop().expect("one element"),
                    id: id.clone(),
                }
                .hooked();
                let mut stand = match width {
                    AvailableSpace::Definite(w) => div().block().w(w).child(probe),
                    _ => div().block().child(probe),
                }
                .into_any_element();
                stand.layout_as_root(size(width, AvailableSpace::MaxContent), window, cx);
                let id = id.get()?;
                let snapshot = match width {
                    AvailableSpace::Definite(_) => Some(window.layout_snapshot(id)),
                    _ => None,
                };
                // Unrounded, as the built block gives its parent.
                Some((window.layout_size_unrounded(id), snapshot))
            })
        })
    });
    counters.write(&source.opts.cx);
    let Some((laid, snapshot)) = laid_out else {
        slot.failed.set(true);
        return MeasuredContent::new(size(px(0.0), px(0.0)));
    };
    match snapshot {
        Some(Some(snapshot)) => slot.measured.set(Some(snapshot)),
        Some(None) => slot.failed.set(true),
        None => {}
    }
    let snapshot = snapshot.flatten();
    slot.sizes.borrow_mut().push((width, laid, snapshot));
    content(laid, snapshot)
}

/// A leaf of size `laid` standing for the block whose exact leaf is
/// `snapshot`: its block container collapses the block's margins and unions
/// its overflow as the built block's.
fn content(laid: Size<Pixels>, snapshot: Option<gpui::LayoutSnapshot>) -> MeasuredContent {
    match snapshot {
        Some(snapshot) => MeasuredContent::standing_for(laid, &snapshot),
        None => MeasuredContent::new(laid),
    }
}

/// The measure of a measured leaf (`measure`).
pub(super) struct LeafMeasure {
    pub(super) source: Rc<Source>,
    pub(super) slot: Rc<Slot>,
}

impl gpui::CompactMeasure for LeafMeasure {
    type Compact = Self;

    fn measure(
        &mut self,
        known: Size<Option<Pixels>>,
        available: Size<AvailableSpace>,
        _: gpui::MeasuredBox,
        window: &mut Window,
        cx: &mut App,
    ) -> MeasuredContent {
        measure(&self.source, &self.slot, known, available, window, cx)
    }

    fn compact(&mut self) -> Option<Self> {
        None
    }
}
