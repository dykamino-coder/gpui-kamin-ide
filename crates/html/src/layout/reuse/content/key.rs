//! The input of a content from outside its container's subtree, and the key of its records.

use crate::dom::{BoxId, Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

/// The container's own identity in a content key (ContentKey::ids).
pub(super) fn ids(s: &BoxStyle) -> [u64; 4] {
    [s.self_node, s.cb_node, s.fixed_cb_node, s.decor_box]
}

/// The input of a content from outside its container's subtree: the container's own
/// identity (`ids`), the widths it gets, and an environment that equal
/// containers (the cells of a table column) share as one value (`KeyInputs::env`).
pub(crate) struct ContentKey {
    pub(super) env: Rc<KeyEnv>,
    /// `self_node`, `cb_node`, `fixed_cb_node` and `decor_box` of the container.
    pub(super) ids: [u64; 4],
    pub(super) len: u32,
    /// Containing-block and available widths of the content (`key_widths`).
    pub(super) widths: (Option<f32>, Option<f32>),
}

/// What a container's content reads from outside its subtree, apart from
/// the container's identity and widths.
///
/// The viewport width is not in it: an inert subtree (`inert_tree`) has no
/// viewport lengths and no box placed by the viewport, so it reads the window
/// width only through the widths it gets, which count when the build read
/// them (`BoxConstraints::track_widths_below`). This is the root flow's rule
/// for a block (`ReplayRecord::fits`), and it lets the content of a cell whose
/// column keeps its width replay while the window is resized; the frame check
/// still refuses a replay whose box got another size.
#[derive(Clone)]
pub(crate) struct KeyEnv {
    pub(super) inherited: Rc<BoxStyle>,
    pub(super) avail_h: Option<f32>,
    /// The content is a table cell's own block list (`BoxConstraints`).
    pub(super) cell_bfc: bool,
    /// The container itself is a cell's block formatting context root.
    pub(super) outer_cell_bfc: bool,
    pub(super) fresh: bool,
    pub(super) root: Rc<BoxStyle>,
    pub(super) viewport_height: f32,
    pub(super) text: Rc<gpui::TextStyle>,
    pub(super) normal_line_height: f32,
    pub(super) fonts: u64,
}

impl KeyEnv {
    pub(super) fn of(
        inputs: &super::shared::KeyInputs,
        inherited: &BoxStyle,
        outer: &RenderOpts,
        inner: &RenderOpts,
        fresh: bool,
    ) -> Self {
        let c = &inner.cx.constraints;
        Self {
            inherited: inputs.style(inherited),
            avail_h: c.avail_h,
            cell_bfc: c.cell_bfc,
            outer_cell_bfc: outer.cx.constraints.cell_bfc,
            fresh,
            root: inputs.root(&inner.root_style()),
            viewport_height: inner.viewport.1,
            text: inputs.text(&inner.text),
            normal_line_height: inner.normal_line_height,
            fonts: inner.cx.doc().values.font_generation(),
        }
    }

    pub(super) fn same(&self, other: &Self) -> bool {
        let (a, b) = (&self.inherited, &other.inherited);
        (Rc::ptr_eq(a, b) || a.same_box_values(b))
            && self.avail_h == other.avail_h
            && self.cell_bfc == other.cell_bfc
            && self.outer_cell_bfc == other.outer_cell_bfc
            && self.fresh == other.fresh
            && (Rc::ptr_eq(&self.root, &other.root) || **self.root == **other.root)
            && self.viewport_height == other.viewport_height
            && (Rc::ptr_eq(&self.text, &other.text) || *self.text == *other.text)
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }
}

impl ContentKey {
    /// The same input; the widths count if the recording build read them.
    pub(super) fn same(&self, other: &Self, read_widths: bool) -> bool {
        (Rc::ptr_eq(&self.env, &other.env) || self.env.same(&other.env))
            && self.ids == other.ids
            && self.len == other.len
            && (!read_widths || self.widths == other.widths)
    }
}

/// The key of the records and bans of the content of box `id`: apart from
/// the node ids that key the root flow's records (the top bit).
pub(super) fn key(id: BoxId) -> u64 {
    1 << 63 | u64::from(id.generation) << 32 | u64::from(id.slot)
}

/// A build without fragmentation, line clamping, ruby or deferred out-of-flow
/// builds; the subtree is inert (`inert_tree`) and its styles do not follow
/// the window (`window_free`).
pub(super) fn plain(nodes: &[Node], inherited: &BoxStyle, opts: &RenderOpts) -> bool {
    let cx = &opts.cx;
    cx.fragmentation.lines.is_none()
        && cx.clamp.input.is_none()
        && cx.ruby.base.is_none()
        && cx.inline.trim_requests.is_empty()
        && cx.position.oof_identities.is_none()
        && no_query_container(
            inherited.reset_context.cq_size,
            inherited.reset_context.cq_box,
        )
        && nodes.iter().all(|n| match n {
            Node::Element(e) => inert_tree(e, cx.vcx()) && window_free(e),
            Node::Text(_) => true,
        })
}

/// No style of the subtree of `e` folded a viewport length at the cascade
/// (`ComputedSlot::reads_viewport`: a `font-size: 2vw` is pixels by now, and
/// a new window width restyles the element without a layout epoch) and none
/// sees a query container (container lengths and queries follow its size).
fn window_free(e: &Element) -> bool {
    super::super::plain::all_elements(e, |e| {
        let context = &e.style.used().reset_context;
        !e.style.slot().reads_viewport() && no_query_container(context.cq_size, context.cq_box)
    })
}

/// No query container size reaches the style (ResetContext::cq_size, cq_box).
fn no_query_container(
    size: (Option<f32>, Option<f32>),
    cq_box: (Option<f32>, Option<f32>),
) -> bool {
    size == (None, None) && cq_box == (None, None)
}

/// Every box of the subtree of `e` is inert (`replayable_node`): no box
/// positions, floats, transforms, blends or takes a paint key or anchor its
/// output keeps, so its content replays with moved counters (`content`).
pub(in crate::layout::reuse) fn inert_tree(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    super::super::plain::all_elements(e, |e| {
        super::super::plain::replayable_node(e, vcx)
            && e.style.used().reset_positioned.anchor_name.is_none()
            && e.style.used().reset_positioned.implicit_anchor.is_none()
    })
}
