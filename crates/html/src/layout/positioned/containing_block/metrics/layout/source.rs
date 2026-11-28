//! Исходная native-раскладка: обычный flow либо page area с собственным ICB.

use super::super::{Snapshot, record::Recorder};
use super::{any_box, contains_oof_box};
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{
    App, AvailableSpace, Bounds, Div, IntoElement, ParentElement, Styled, Window, div, px, size,
};
use std::rc::Rc;
mod children;
use children::Children;

pub(crate) fn collect(
    kids: &[Element],
    parent: &BoxStyle,
    width: f32,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    collect_in(
        Children::Elements(kids),
        parent,
        width,
        None,
        opts,
        window,
        cx,
    )
}

/// A host's child list in normal flow, which records direct positioned children's static positions.
/// `host_box`: the host's content box (inline size, definite block size) when the flow is
/// narrower than the host (a column): the host, not the column, is the containing block of
/// its direct positioned children (CSS 2.1 §10.1, css-multicol-1 §2).
pub(crate) fn collect_nodes(
    kids: &[Node],
    parent: &BoxStyle,
    width: f32,
    host_box: Option<(f32, Option<f32>)>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    let kids = Children::Nodes(kids, host_box);
    collect_in(kids, parent, width, None, opts, window, cx)
}

pub(crate) fn collect_paged(
    kids: &[Node],
    parent: &BoxStyle,
    page: ((f32, f32), &dyn Fn() -> Div),
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    collect_in(
        Children::Nodes(kids, None),
        parent,
        page.0.0,
        Some((page.0.1, page.1)),
        opts,
        window,
        cx,
    )
}

fn collect_in(
    kids: Children<'_>,
    parent: &BoxStyle,
    width: f32,
    page: Option<(f32, &dyn Fn() -> Div)>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
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
        build(&kids, parent, width, page, opts, window, cx);
    if settle {
        let settling = Rc::new(super::super::record::Settling(recorder.clone()));
        let layout = opts.cx.layout().clone();
        let missed = crate::layout::positioned::anchor::misses_of(&layout.anchors, || {
            let _ = window.transact(|window| {
                window.with_render_context(Some(settling), |window| {
                    root.prepaint_at(gpui::point(px(0.), px(0.)), window, cx)
                });
                Err::<(), ()>(())
            });
        });
        recorder.unsettled.replace(missed);
    }
    let mut snapshot = recorder.snapshot(window);
    snapshot.host = if is_page { 0 } else { parent.self_node };
    recorder.fragment_snapshot(&mut snapshot, window);
    let (root, layout, root_size) = if settle {
        let (_, root, layout, size) = build(&kids, parent, width, page, opts, window, cx);
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
    any_box(e, |e| {
        crate::layout::positioned::anchor::resolve::needs_deferred(&e.style.used())
    })
}

/// One source tree laid out at `width` with its own recorder.
#[allow(clippy::too_many_arguments)]
fn build(
    kids: &Children<'_>,
    parent: &BoxStyle,
    width: f32,
    page: Option<(f32, &dyn Fn() -> Div)>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> (
    Rc<Recorder>,
    gpui::AnyElement,
    crate::layout::fragment::geometry::Node,
    gpui::Size<gpui::Pixels>,
) {
    let host_box = match kids {
        Children::Nodes(_, host_box) if page.is_none() => *host_box,
        _ => None,
    };
    let mut recorder = Recorder::for_fragments(kids.elements(), parent, opts);
    if host_box.is_some() {
        recorder.host = parent.self_node;
    }
    let recorder = Rc::new(recorder);
    let mut opts = opts.clone();
    opts.cx.position = Default::default();
    opts.cx.deferred = Default::default();
    opts.cx.position.geometry_record = Some(recorder.clone());
    opts.cx.position.geometry = None;
    opts.cx.constraints.set_avail_w(Some(width));
    opts.frag = Default::default();
    // Начальная page area — настоящий ICB; normal потомки остаются overflow.
    let icb = page.as_ref().map(|_| {
        crate::layout::positioned::containing_block::LayerScope::icb(&opts.cx.position.state)
    });
    let paged = page
        .as_ref()
        .map(|_| crate::layout::page::paged::PagedBuild::begin());
    opts.frag.paged = paged.clone();
    let host_cb = (page.is_none()
        && matches!(kids, Children::Nodes(..))
        && crate::text::inline::establishes_cb(parent))
    .then(|| {
        crate::layout::positioned::containing_block::LayerScope::cb(
            &opts.cx.position.state,
            crate::style::computed::stacking::fixed_cb_layer_box(parent),
            None,
        )
    });
    let children = kids.build(parent, &opts);
    let layer = host_cb.map(|scope| scope.finish()).unwrap_or_default();
    let is_page = page.is_some();
    let mut frame = match (page, host_box) {
        (Some((height, frame)), _) => frame()
            .relative()
            .w(px(width))
            .h(px(height))
            .children(children)
            .children(layer),
        // The flow keeps the column's width; the host's layer lays the
        // host's direct positioned children out in the host's own box.
        (None, Some((inline, block))) => {
            let host = div().relative().w(px(inline));
            let host = match block {
                Some(block) => host.h(px(block)),
                None => host,
            };
            host.child(div().block().w(px(width)).children(children))
                .children(layer)
        }
        (None, None) => div()
            .block()
            .w(px(width))
            .children(children)
            .children(layer),
    };
    if let Some(icb) = icb {
        frame = frame
            .children(icb.finish())
            .children(opts.take_paged_fixed());
    }
    if let Some(paged) = paged {
        paged.end();
    }
    let mut root = frame.into_any_element();
    // Нулевой owner — прямой OOF, а настоящий id родителя нужен его CB.
    // Обе метки ссылаются на тот же LayoutId без дополнительной коробки.
    root = super::super::record::wrap(0, parent, root, &opts);
    if !is_page && parent.self_node != 0 {
        root = super::super::record::wrap(parent.self_node, parent, root, &opts);
    }
    let layout =
        crate::layout::fragment::geometry::Node::capture(root.request_layout(window, cx), window);
    let root_size = root.layout_as_root(
        size(
            AvailableSpace::Definite(px(width)),
            AvailableSpace::MaxContent,
        ),
        window,
        cx,
    );
    (recorder, root, layout, root_size)
}
