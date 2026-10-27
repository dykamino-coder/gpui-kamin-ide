//! Исходная native-раскладка: обычный flow либо page area с собственным ICB.

use super::super::{Snapshot, record::Recorder};
use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::{App, AvailableSpace, Div, IntoElement, ParentElement, Styled, Window, div, px, size};
use std::rc::Rc;

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

pub(crate) fn collect_paged(
    kids: &[Node],
    parent: &BoxStyle,
    page: ((f32, f32), Div),
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    collect_in(
        Children::Nodes(kids),
        parent,
        page.0.0,
        Some((page.0.1, page.1)),
        opts,
        window,
        cx,
    )
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

fn collect_in(
    kids: Children<'_>,
    parent: &BoxStyle,
    width: f32,
    page: Option<(f32, Div)>,
    opts: &RenderOpts,
    window: &mut Window,
    cx: &mut App,
) -> Rc<Snapshot> {
    if !kids.elements().any(contains_positioned) {
        return Rc::default();
    }
    window.with_nested_layout(|window| {
        let recorder = Rc::new(Recorder::for_fragments(
            kids.elements(),
            parent,
            opts,
            page.is_some() || opts.frag.paged.is_some(),
        ));
        let mut opts = opts.clone();
        opts.cx.position = Default::default();
        opts.cx.deferred = Default::default();
        opts.cx.position.geometry_record = Some(recorder.clone());
        opts.cx.position.geometry = None;
        opts.cx.constraints.avail_w = Some(width);
        opts.frag = Default::default();
        // Начальная page area — настоящий ICB; normal потомки остаются overflow.
        let icb = page.as_ref().map(|_| {
            crate::layout::positioned::containing_block::LayerScope::icb(&opts.cx.position.state)
        });
        let paged = page
            .as_ref()
            .map(|_| crate::layout::page::paged::PagedBuild::begin());
        opts.frag.paged = paged.clone();
        let children = kids.build(parent, &opts);
        let is_page = page.is_some();
        let mut frame = match page {
            Some((height, frame)) => frame
                .relative()
                .w(px(width))
                .h(px(height))
                .children(children),
            None => div().block().w(px(width)).children(children),
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
        if is_page {
            root = super::super::record::wrap(0, parent, root, &opts);
        }
        root.layout_as_root(
            size(
                AvailableSpace::Definite(px(width)),
                AvailableSpace::MaxContent,
            ),
            window,
            cx,
        );
        let mut snapshot = recorder.snapshot(window);
        recorder.fragment_snapshot(&mut snapshot, window);
        Rc::new(snapshot)
    })
}
