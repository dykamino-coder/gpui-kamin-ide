//! Вход содержимого снаружи поддерева контейнера и ключ его записей.

use crate::dom::{BoxId, Element, Node};
use crate::layout::cx::BoxConstraints;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;

/// Вход содержимого снаружи поддерева контейнера.
pub(crate) struct ContentKey {
    pub(super) inherited: BoxStyle,
    pub(super) constraints: BoxConstraints,
    pub(super) cell_bfc: bool,
    pub(super) fresh: bool,
    pub(super) len: usize,
    pub(super) root: BoxStyle,
    pub(super) viewport: (f32, f32),
    pub(super) text: gpui::TextStyle,
    pub(super) normal_line_height: f32,
    pub(super) fonts: u64,
}

impl ContentKey {
    pub(super) fn same(&self, other: &Self) -> bool {
        let (a, b) = (&self.inherited, &other.inherited);
        a.same_box_values(b)
            && a.self_node == b.self_node
            && a.cb_node == b.cb_node
            && a.fixed_cb_node == b.fixed_cb_node
            && a.decor_box == b.decor_box
            && self.constraints == other.constraints
            && self.constraints.key_widths() == other.constraints.key_widths()
            && self.cell_bfc == other.cell_bfc
            && self.fresh == other.fresh
            && self.len == other.len
            && *self.root == *other.root
            && self.viewport == other.viewport
            && self.text == other.text
            && self.normal_line_height == other.normal_line_height
            && self.fonts == other.fonts
    }
}

/// The key of the records and bans of the content of box `id`: apart from
/// the node ids that key the root flow's records (the top bit).
pub(super) fn key(id: BoxId) -> u64 {
    1 << 63 | u64::from(id.generation) << 32 | u64::from(id.slot)
}

/// Сборка без фрагментации, обрезки строк, руби и отложенных внепоточных
/// сборок; поддерево инертно (`inert_tree`).
pub(super) fn plain(nodes: &[Node], opts: &RenderOpts) -> bool {
    let cx = &opts.cx;
    cx.fragmentation.lines.is_none()
        && cx.clamp.input.is_none()
        && cx.ruby.base.is_none()
        && cx.inline.trim_requests.is_empty()
        && cx.position.oof_identities.is_none()
        && nodes.iter().all(|n| match n {
            Node::Element(e) => inert_tree(e, cx.vcx()),
            Node::Text(_) => true,
        })
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
