//! The elements of `offscreen`: a measured leaf, an exact leaf and a watch.

use super::measure::{LeafMeasure, Source};
use super::{Kind, Slot};
use crate::dom::{Element, Node};
use crate::hooks::{Hooks, IntoHooked, Wrap};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Refineable, Styled, Window, div};
use std::rc::Rc;

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
    let mut refined = gpui::Style::default();
    refined.refine(d.style());
    let style = gpui::Style {
        display: gpui::Display::Block,
        flex_shrink: 0.0,
        ..refined
    };
    let mut block = e.clone();
    crate::layout::block::margins::strip_inline_margins(&mut block);
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

impl Hooks for Measured {
    fn paint_boundary(&self) -> bool {
        false
    }

    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, _: &mut App) -> (LayoutId, ()) {
        let measure = LeafMeasure {
            source: self.source.clone(),
            slot: self.slot.clone(),
        };
        let id = window.request_measured_layout_with_measure(self.style.clone(), measure);
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
