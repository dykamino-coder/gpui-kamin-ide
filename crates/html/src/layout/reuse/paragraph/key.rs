//! What a kept paragraph read from outside its nodes, and the conditions of
//! keeping it (`super`).

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// The input of a paragraph from outside its nodes, less the available
/// width: the line layout reads that itself at every layout.
pub(super) struct ParaKey {
    pub(super) inherited: Rc<BoxStyle>,
    ids: [u64; 4],
    avail_h: Option<f32>,
    cell_bfc: bool,
    text: Rc<gpui::TextStyle>,
    normal_line_height: f32,
    fonts: u64,
    viewport: (f32, f32),
    /// A style of the paragraph's nodes reads the viewport size: its width
    /// is an input too.
    reads_viewport: bool,
}

impl ParaKey {
    pub(super) fn of(
        inputs: &super::super::content::shared::KeyInputs,
        inherited: &BoxStyle,
        opts: &RenderOpts,
        reads_viewport: bool,
    ) -> Self {
        let cx = &opts.cx;
        Self {
            ids: ids(inherited),
            inherited: inputs.style(inherited),
            avail_h: cx.constraints.avail_h,
            cell_bfc: cx.constraints.cell_bfc,
            text: inputs.text(&opts.text),
            normal_line_height: opts.normal_line_height,
            fonts: cx.doc().values.font_generation(),
            viewport: opts.viewport,
            reads_viewport,
        }
    }

    /// The paragraph of `inherited` under `opts` has this input.
    pub(super) fn holds(&self, inherited: &BoxStyle, opts: &RenderOpts) -> bool {
        let cx = &opts.cx;
        self.ids == ids(inherited)
            && self.avail_h == cx.constraints.avail_h
            && self.cell_bfc == cx.constraints.cell_bfc
            && self.normal_line_height == opts.normal_line_height
            && self.fonts == cx.doc().values.font_generation()
            && self.viewport.1 == opts.viewport.1
            && (!self.reads_viewport || self.viewport.0 == opts.viewport.0)
            && *self.text == opts.text
            && self.inherited.same_box_values(inherited)
    }
}

fn ids(s: &BoxStyle) -> [u64; 4] {
    [s.self_node, s.cb_node, s.fixed_cb_node, s.decor_box]
}

/// The record key of the paragraph of `nodes` in container `self_node`: the
/// keys of its top-level elements and the lengths of its texts. A changed
/// node inside moves the container's build epoch, not this key.
pub(super) fn id(self_node: u64, nodes: &[Node]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    self_node.hash(&mut h);
    nodes.len().hash(&mut h);
    for n in nodes {
        match n {
            Node::Element(e) => e.node_id.hash(&mut h),
            Node::Text(t) => t.as_str().len().hash(&mut h),
        }
    }
    h.finish()
}

/// The build context holds nothing that a later frame would not give the
/// paragraph again: no fragmentation, line clamp, ruby, line trim, deferred
/// out-of-flow boxes or measure of an enclosing inline or clamp.
pub(super) fn plain(inherited: &BoxStyle, opts: &RenderOpts) -> bool {
    let cx = &opts.cx;
    cx.fragmentation.lines.is_none()
        && cx.clamp.input.is_none()
        && cx.clamp.paragraph.is_none()
        && cx.clamp.atom_depth == 0
        && cx.clamp_measure().is_none()
        && cx.ruby.base.is_none()
        && cx.ruby.layers.is_none()
        && cx.inline.trim_requests.is_empty()
        && cx.inline.inline_measure.is_none()
        && crate::text::line_trim::para(cx.inline.trim_para, inherited.self_node)
            .is_none_or(|t| t == crate::text::line_trim::Trim::default())
        && cx.position.oof_identities.is_none()
        && opts.frag.watch.is_none()
}

/// Every top-level element of the paragraph is inert (`inert_tree`); whether
/// a style below reads the viewport size. `None` if one is not inert.
pub(super) fn subtree(nodes: &[Node], opts: &RenderOpts) -> Option<bool> {
    let vcx = opts.cx.vcx();
    let mut reads = false;
    for n in nodes {
        let Node::Element(e) = n else { continue };
        // A kept paragraph keeps no note of the hover it paints.
        if !super::super::content::inert_tree(e, vcx)
            || !super::super::plain::all_elements(e, super::super::plain::hover_free)
        {
            return None;
        }
        reads = reads
            || !super::super::plain::all_elements(e, |e| !e.style.used().has_viewport_lengths(vcx));
    }
    Some(reads)
}

/// Atom `e` has its own inline size in px and no margin, padding or height
/// of a percentage of the containing block: its box is the same at every
/// available width (`atom_inline_size_fixed` decides the rest).
pub(super) fn fixed_edges(e: &Element) -> bool {
    let style = e.style.used();
    let b = &style.reset_box_layout;
    let fixed = |l: Option<Len>| !matches!(l, Some(Len::Pct(_) | Len::Calc(_)));
    let (m, p) = (&b.margin, &b.padding);
    let px_or_none = |l: Option<Len>| matches!(l, None | Some(Len::Px(_)));
    [
        m.top, m.right, m.bottom, m.left, p.top, p.right, p.bottom, p.left,
    ]
    .into_iter()
    .all(fixed)
        && !matches!(b.height, Some(Len::Pct(_) | Len::Calc(_)))
        && matches!(b.width, Some(Len::Px(_)))
        && px_or_none(b.min_width)
        && px_or_none(b.max_width)
}
