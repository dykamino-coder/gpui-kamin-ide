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
    /// How deep in inline atoms the paragraph lies (`ClampCx::atom_depth`).
    atom_depth: u32,
    /// The content of an atom is built for the atom's own constraint (its widths,
    /// `key_widths`): if the build read it (`plan::Widths`), the record holds only
    /// at the same one; `None` — not an input.
    atom_widths: Option<(Option<f32>, Option<f32>)>,
}

fn atom_widths(cx: &crate::layout::cx::LayoutCx) -> Option<(Option<f32>, Option<f32>)> {
    (cx.clamp.atom_depth > 0).then(|| cx.constraints.key_widths())
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
            atom_depth: cx.clamp.atom_depth,
            atom_widths: atom_widths(cx),
        }
    }

    /// The build read no width of the atom it lies in: the atom's constraint is
    /// no input of the record.
    pub(super) fn width_free(&mut self) {
        self.atom_widths = None;
    }

    /// The first input of the paragraph of `inherited` under `opts` that is
    /// not this one, by name; `None`: the record holds.
    pub(super) fn changed(&self, inherited: &BoxStyle, opts: &RenderOpts) -> Option<&'static str> {
        let cx = &opts.cx;
        let same = [
            (self.ids == ids(inherited), "ids"),
            (self.atom_depth == cx.clamp.atom_depth, "atom_depth"),
            (
                self.atom_widths.is_none() || self.atom_widths == atom_widths(cx),
                "atom_widths",
            ),
            (self.avail_h == cx.constraints.avail_h, "avail_h"),
            (self.cell_bfc == cx.constraints.cell_bfc, "cell_bfc"),
            (
                self.normal_line_height == opts.normal_line_height,
                "normal_line_height",
            ),
            (self.fonts == cx.doc().values.font_generation(), "fonts"),
            (self.viewport.1 == opts.viewport.1, "viewport_height"),
            (
                !self.reads_viewport || self.viewport.0 == opts.viewport.0,
                "viewport_width",
            ),
            (*self.text == opts.text, "text_style"),
        ];
        if let Some(&(_, name)) = same.iter().find(|(same, _)| !same) {
            return Some(name);
        }
        (!self.inherited.same_box_values(inherited)).then_some("inherited")
    }
}

fn ids(s: &BoxStyle) -> [u64; 4] {
    [s.self_node, s.cb_node, s.fixed_cb_node, s.decor_box]
}

/// The record key of the paragraph of `nodes` in container `self_node`: the
/// keys of its top-level elements, the source text node each text starts with
/// (the text itself if it has none) and the lengths of the texts, so two runs of
/// one block differ. A changed node inside moves the container's build epoch,
/// not this key.
pub(super) fn id(self_node: u64, nodes: &[Node]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    self_node.hash(&mut h);
    nodes.len().hash(&mut h);
    for n in nodes {
        match n {
            Node::Element(e) => e.node_id.hash(&mut h),
            Node::Text(t) => {
                t.as_str().len().hash(&mut h);
                match t.first_source() {
                    Some(source) => source.hash(&mut h),
                    None => t.as_str().hash(&mut h),
                }
            }
        }
    }
    h.finish()
}

/// The build context holds nothing that a later frame would not give the
/// paragraph again: no fragmentation, line clamp, ruby, line trim, deferred
/// out-of-flow boxes or measure of an enclosing inline or clamp.
/// The first condition that fails names the refusal (`super::Refusal`).
pub(super) fn plain(inherited: &BoxStyle, opts: &RenderOpts) -> Result<(), super::Refusal> {
    use super::Refusal;
    let cx = &opts.cx;
    // Without a clamp container above (`clamp_context`) the build reads no clamp state; the
    // depth of atoms it lies in is a key input (`ParaKey::atom_depth`), not a refusal.
    let clamp = cx.clamp.input.is_none()
        && cx.clamp.paragraph.is_none()
        && crate::text::clamp::clamp_context(cx).is_none();
    let trim = cx.inline.trim_requests.is_empty()
        && crate::text::line_trim::para(cx.inline.trim_para, inherited.self_node)
            .is_none_or(|t| t == crate::text::line_trim::Trim::default());
    match () {
        _ if cx.fragmentation.lines.is_some() || opts.frag.watch.is_some() => {
            Err(Refusal::Fragmented)
        }
        _ if !clamp => Err(Refusal::Clamp),
        _ if cx.ruby.base.is_some() || cx.ruby.layers.is_some() => Err(Refusal::Ruby),
        _ if !trim => Err(Refusal::Trim),
        _ if cx.inline.inline_measure.is_some() => Err(Refusal::InlineMeasure),
        _ if cx.position.oof_identities.is_some() => Err(Refusal::OutOfFlow),
        _ => Ok(()),
    }
}

/// Every top-level element of the paragraph is inert (`inert_tree`); whether
/// a style below reads the viewport size; the refusal if one is not inert or paints a hover.
pub(super) fn subtree(nodes: &[Node], opts: &RenderOpts) -> Result<bool, super::Refusal> {
    let vcx = opts.cx.vcx();
    let mut reads = false;
    for n in nodes {
        let Node::Element(e) = n else { continue };
        if !super::super::plain::all_elements(e, |e| inert(e, vcx)) {
            crate::why::note("paragraph.subtree", || {
                format!("{:?}", super::super::plain::not_inert(e, vcx))
            });
            return Err(super::Refusal::Subtree);
        }
        // A kept paragraph keeps no note of the hover it paints.
        if !super::super::plain::all_elements(e, super::super::plain::hover_free) {
            return Err(super::Refusal::Hover);
        }
        // A style that folded a viewport length at the cascade (`font-size: 1vw` is pixels by
        // now) changes at a new window width without a build epoch: it reads the width too.
        reads = reads
            || !super::super::plain::all_elements(e, |e| {
                !e.style.used().has_viewport_lengths(vcx) && !e.style.slot().reads_viewport()
            });
    }
    Ok(reads)
}

/// Box `e` of a kept paragraph is inert (`content::inert_node`). Every
/// pseudo-element names its originating element as its implicit anchor; only a
/// box that refers to it (`position-anchor: auto`, a `position-area`) reads
/// another box's geometry, and an in-flow one is not anchor-positioned at all.
fn inert(e: &Element, vcx: crate::values::ValueCx<'_>) -> bool {
    let style = e.style.used();
    let positioned = &style.reset_positioned;
    super::super::plain::replayable_node(e, vcx)
        && positioned.anchor_name.is_none()
        && (positioned.implicit_anchor.is_none()
            || !crate::layout::positioned::anchor::wants_implicit(&style))
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
