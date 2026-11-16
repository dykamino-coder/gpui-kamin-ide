//! Вход содержимого снаружи поддерева контейнера и ключ его записей.

use crate::dom::{BoxId, Element, Node};
use crate::layout::cx::BoxConstraints;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use std::rc::Rc;

/// The container's own identity in a content key (ContentKey::ids).
pub(super) fn ids(s: &BoxStyle) -> [u64; 4] {
    [s.self_node, s.cb_node, s.fixed_cb_node, s.decor_box]
}

/// Вход содержимого снаружи поддерева контейнера. The styles are shared by
/// the keys of equal containers (`KeyInputs`); the container's own identity
/// is kept apart (`ids`).
pub(crate) struct ContentKey {
    pub(super) inherited: Rc<BoxStyle>,
    /// `self_node`, `cb_node`, `fixed_cb_node` and `decor_box` of the container.
    pub(super) ids: [u64; 4],
    pub(super) constraints: BoxConstraints,
    pub(super) cell_bfc: bool,
    pub(super) fresh: bool,
    pub(super) len: usize,
    pub(super) root: Rc<BoxStyle>,
    pub(super) viewport: (f32, f32),
    pub(super) text: Rc<gpui::TextStyle>,
    pub(super) normal_line_height: f32,
    pub(super) fonts: u64,
}

impl ContentKey {
    pub(super) fn same(&self, other: &Self) -> bool {
        let (a, b) = (&self.inherited, &other.inherited);
        (Rc::ptr_eq(a, b) || a.same_box_values(b))
            && self.ids == other.ids
            && self.constraints == other.constraints
            && self.constraints.key_widths() == other.constraints.key_widths()
            && self.cell_bfc == other.cell_bfc
            && self.fresh == other.fresh
            && self.len == other.len
            && (Rc::ptr_eq(&self.root, &other.root) || **self.root == **other.root)
            && self.viewport == other.viewport
            && (Rc::ptr_eq(&self.text, &other.text) || *self.text == *other.text)
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
