//! The elements of `offscreen`: a measured leaf, an exact leaf and a watch.

use super::{Kind, Slot};
use crate::dom::{Element, Node};
use crate::hooks::{Hooks, IntoHooked, Wrap};
use crate::layout::options::RenderOpts;
use crate::layout::reuse::Counters;
use crate::style::computed::BoxStyle;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, IntoElement, LayoutId, ParentElement, Pixels,
    Refineable, Size, Styled, Window, div, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

/// What the measure of a far block builds it from.
struct Source {
    /// The block without its inline margins: its leaf has them.
    node: Node,
    inherited: BoxStyle,
    /// The block's options without the document's records: the private build
    /// registers nothing with the frame.
    opts: RenderOpts,
}

struct Measured {
    slot: Rc<Slot>,
    source: Rc<Source>,
    style: gpui::Style,
}

struct Exact {
    slot: Rc<Slot>,
    snapshot: gpui::LayoutSnapshot,
}

/// The leaf element of `slot` for block `e` of container `inherited`.
pub(super) fn element(
    slot: Rc<Slot>,
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    if let Kind::Exact(snapshot) = slot.kind {
        return Exact { slot, snapshot }.hooked();
    }
    let vcx = opts.cx.vcx();
    let margins = &e.style.used().reset_box_layout.margin;
    let mut d = crate::paint::apply::margins(div(), margins, vcx);
    let mut style = gpui::Style::default();
    style.refine(d.style());
    style.display = gpui::Display::Block;
    style.flex_shrink = 0.0;
    let mut block = e.clone();
    let own = &mut block.style.used_mut().reset_box_layout.margin;
    own.left = None;
    own.right = None;
    let opts = opts.inner(|cx| {
        cx.contracts.reuse = None;
        cx.contracts.offscreen_off = true;
    });
    let source = Rc::new(Source {
        node: Node::Element(block),
        inherited: inherited.clone(),
        opts: (*opts).clone(),
    });
    Measured {
        slot,
        source,
        style,
    }
    .hooked()
}

/// `child`, the built block of `slot`, whose layout node the frame check reads.
pub(super) fn watch(slot: Rc<Slot>, child: AnyElement) -> AnyElement {
    gpui::note_layoutless_element();
    Watch { slot, child }.hooked()
}

struct Watch {
    slot: Rc<Slot>,
    child: AnyElement,
}

impl Wrap for Watch {
    fn paint_boundary(&self) -> bool {
        false
    }

    fn paint_metadata(&self) -> Option<&dyn std::any::Any> {
        self.child.paint_metadata()
    }

    fn dispatch_node(&self) -> bool {
        false
    }

    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        self.slot.place(id, window);
        id
    }
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
) -> Size<Pixels> {
    let width = known
        .width
        .map_or(available.width, AvailableSpace::Definite);
    // Layout asks a leaf for the same width more than once (intrinsic
    // sizes, then the final pass): the block is built once per width.
    let cached = slot.sizes.borrow().iter().find(|s| s.0 == width).copied();
    if let Some((_, laid, snapshot)) = cached {
        slot.measured.set(snapshot.or(slot.measured.get()));
        return laid;
    }
    if slot.failed.get() {
        return Size::default();
    }
    let counters = Counters::read(&source.opts.cx);
    let laid_out = gpui::with_temporary_element_arena(|| {
        window.with_nested_layout(|window| {
            window.with_line_layouts_released(|window| {
                let opts = &source.opts;
                let build = &opts.cx.contracts.build;
                let mut kids =
                    build.blocks(std::slice::from_ref(&source.node), &source.inherited, opts);
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
        return size(px(0.0), px(0.0));
    };
    match snapshot {
        Some(Some(snapshot)) => slot.measured.set(Some(snapshot)),
        Some(None) => slot.failed.set(true),
        None => {}
    }
    let snapshot = snapshot.flatten();
    slot.sizes.borrow_mut().push((width, laid, snapshot));
    laid
}

impl Hooks for Measured {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _: &mut App) -> (LayoutId, ()) {
        let (source, slot) = (self.source.clone(), self.slot.clone());
        let id = window.request_measured_layout(
            self.style.clone(),
            move |known, available, window, cx| {
                measure(&source, &slot, known, available, window, cx)
            },
        );
        self.slot.place(id, window);
        (id, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

impl Hooks for Exact {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        if self.snapshot.scale() != window.scale_factor() {
            // A leaf of another scale is not exact: the frame check refuses it.
            self.slot.failed.set(true);
            return (window.request_layout(gpui::Style::default(), [], cx), ());
        }
        let id = window.request_snapshot_layout(&self.snapshot.stretched());
        self.slot.place(id, window);
        (id, ())
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {}

    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}
