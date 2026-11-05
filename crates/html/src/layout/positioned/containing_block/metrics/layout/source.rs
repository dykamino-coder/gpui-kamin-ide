//! Исходная native-раскладка: обычный flow либо page area с собственным ICB.

use super::super::{Snapshot, record::Recorder};
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{App, Bounds, Div, Window, px};
use std::rc::Rc;
mod build;
use build::build_async;

pub(crate) fn contains_positioned(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if crate::layout::box_kind::out_of_flow(&e.style.used()) {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            crate::dom::Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    false
}

/// An absolute or fixed box (an independent continuation flow) in the element's subtree.
pub(crate) fn contains_oof_box(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if crate::style::flow::absolutely_positioned(e.style.used().reset_positioned.position) {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            crate::dom::Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    false
}

pub(crate) async fn collect_async(
    kids: &[Element],
    parent: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> Rc<Snapshot> {
    collect_in(
        Children::Elements(kids),
        parent,
        (width, None),
        opts,
        window,
        cx,
        stack,
    )
    .await
}

pub(crate) async fn collect_paged_async(
    kids: &[Node],
    parent: &BoxStyle,
    page: ((f32, f32), &dyn Fn() -> Div),
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> Rc<Snapshot> {
    collect_in(
        Children::Nodes(kids),
        parent,
        (page.0.0, Some((page.0.1, page.1))),
        opts,
        window,
        cx,
        stack,
    )
    .await
}

enum Children<'a> {
    Elements(&'a [Element]),
    Nodes(&'a [Node]),
}

impl Children<'_> {
    fn elements(&self) -> Box<dyn Iterator<Item = &Element> + '_> {
        match self {
            Self::Elements(kids) => Box::new(kids.iter()),
            Self::Nodes(nodes) => Box::new(nodes.iter().filter_map(|n| match n {
                Node::Element(e) => Some(e),
                _ => None,
            })),
        }
    }
    fn build(&self, parent: &BoxStyle, opts: &RenderOpts) -> Vec<gpui::AnyElement> {
        match self {
            Self::Elements(kids) => kids
                .iter()
                .map(|c| opts.cx.contracts.build.element(c, parent, opts))
                .collect(),
            Self::Nodes(nodes) => opts.cx.contracts.build.blocks(nodes, parent, opts),
        }
    }
}

type SourceSpace<'a> = (f32, Option<(f32, &'a dyn Fn() -> Div)>);

async fn collect_in(
    kids: Children<'_>,
    parent: &BoxStyle,
    (width, page): SourceSpace<'_>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> Rc<Snapshot> {
    // Only absolutely positioned boxes take their containing block from this
    // normal layout (CSS 2.1 §10.1) and continue as independent flows; a float
    // stays in its owner's flow (`Recorder::for_fragments`), so a float-only
    // subtree needs no source layout.
    if !kids.elements().any(contains_oof_box) {
        return Rc::default();
    }
    let is_page = page.is_some();
    // Anchor-positioned boxes take their place from anchors published on
    // prepaint: a settling pass places them before the snapshot records their
    // flows (css-anchor-position-1 §3: anchors are laid out first), then rolls
    // its prepaint back; the source that paints is built afresh.
    let settle = kids.elements().any(contains_anchored);
    let (recorder, mut root, layout, root_size) =
        build_async(&kids, parent, width, page, opts, window, cx, stack).await;
    if settle {
        let settling = Rc::new(super::super::record::Settling(recorder.clone()));
        let layout = opts.cx.layout().clone();
        let missed =
            crate::layout::positioned::anchor::misses_of_async(&layout.anchors, async || {
                let _ = window
                    .transact_async(async |window| {
                        window
                            .with_render_context_async(Some(settling), async |window| {
                                root.prepaint_at_async(
                                    gpui::point(px(0.), px(0.)),
                                    window,
                                    cx,
                                    stack,
                                )
                                .await
                            })
                            .await;
                        Err::<(), ()>(())
                    })
                    .await;
            })
            .await;
        recorder.unsettled.replace(missed);
    }
    let mut snapshot = recorder.snapshot(window);
    snapshot.host = if is_page { 0 } else { parent.self_node };
    recorder.fragment_snapshot(&mut snapshot, window);
    let (root, layout, root_size) = if settle {
        let (_, root, layout, size) =
            build_async(&kids, parent, width, page, opts, window, cx, stack).await;
        (root, layout, size)
    } else {
        (root, layout, root_size)
    };
    let extent = snapshot.normal.values().copied().fold(
        Bounds::new(gpui::point(px(0.), px(0.)), root_size),
        |extent, bounds| extent.union(&bounds),
    );
    snapshot.native = Some(Rc::new(super::Native::new(root, layout, extent)));
    Rc::new(snapshot)
}

/// An anchor-positioned box (`anchor::resolve::needs_deferred`) in the subtree.
fn contains_anchored(e: &Element) -> bool {
    let mut pending = vec![e];
    while let Some(e) = pending.pop() {
        if crate::layout::positioned::anchor::resolve::needs_deferred(&e.style.used()) {
            return true;
        }
        pending.extend(e.children.iter().filter_map(|n| match n {
            crate::dom::Node::Element(c) => Some(c),
            _ => None,
        }));
    }
    false
}

#[cfg(test)]
pub(crate) fn collect(
    kids: &[Element],
    parent: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    reblessive::Stack::new()
        .enter(|stack| collect_async(kids, parent, width, opts, window, cx, stack))
        .finish()
}
