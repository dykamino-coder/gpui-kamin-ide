//! The facts of the parent boxes that a child's box stage reads, and only those. The stage
//! reads its parents only through this projection (`box_stage`), so the projection is the
//! complete parent key of the stage memory (`materialize::stage_memo`) by construction: used
//! values the build writes into the parent's style after its stage (the viewport width of
//! `html-viewport`, resolved `vw`, scrollbars) do not touch the child unless they change what
//! it read. Blink decides whether to recompute children the same way, by
//! `StyleRecalcChange` from the difference of the parent's old and new style rather than the
//! equality of the whole style; Stylo by `ChildCascadeRequirement`.

use super::BoxParents;
use crate::box_facts::{AxisSizes, BoxContext};
use crate::style::computed::box_predicates::{backdrop_root, establishes_cb};
use crate::style::computed::orthogonal::InlineConstraint;
use crate::style::computed::writing_mode::{WritingMode, WritingModeKw};
use crate::style::computed::{
    Align, BoxStyle, Decor, Display as D, Filter, InlineFormat, Position as P,
};
use crate::style::decoration_paint::ForegroundPaint;
use crate::style::inline_decoration::InlineDecoration;
use crate::style::values::value::{Color, Len};
use crate::values::ValueCx;
use std::borrow::Cow;
use std::rc::Rc;

/// What a child box stage reads of its parents: of the layout parent (containing block,
/// formatting context, alignment) and of the parent by the element tree (propagated values).
#[derive(Clone, Debug, PartialEq)]
pub struct ParentFacts<'a> {
    pub(super) layout: LayoutFacts,
    pub(super) tree: TreeFacts<'a>,
}

/// The layout parent as the child's containing block and formatting context.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LayoutFacts {
    pub context: BoxContext<WritingMode, Align>,
    /// The values of the child's containing-block facts (`cb_*`, `sc_ancestor`).
    pub cb_ancestor: bool,
    pub cb_depth: u16,
    pub sc_ancestor: bool,
    pub cb_scroll_insets: ([f32; 4], [f32; 4]),
    /// The values of the child's `TreeFacts` that come from this parent.
    pub fragment_clip_ancestor: bool,
    pub in_multicol: bool,
    pub backdrop_root_above: bool,
    pub transform_ancestor: bool,
    pub transformed_ancestor: bool,
    pub orthogonal_scrollport: Option<[AxisSizes; 2]>,
    /// A flex or grid container (`will-change: z-index` stacks its items).
    pub flex_or_grid: bool,
    /// Flex, grid, lanes or a table cell: the parent's layout gives its children a size.
    pub laid_out: bool,
    pub rel_shift: Option<(f32, f32)>,
    pub box_shift: Option<(f32, f32)>,
    /// Is the child's containing block height definite (CSS 2.1 §10.5).
    pub child_cb_height_def: bool,
    /// The quirks percentage basis walk (Quirks Mode §3.5).
    pub quirk_pct_base: Option<f32>,
    pub height: Option<Len>,
    pub ortho_limit: Option<f32>,
    pub orthogonal_inline: Option<InlineConstraint>,
    /// The parent's `line-clamp` when it is a line-clamp container.
    pub line_clamp: Option<u32>,
    /// Grid lanes axes reversed against the writing mode (`flow_align`).
    pub reversed_axes: Option<(bool, bool)>,
}

/// The parent by the element tree: the values propagated along it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TreeFacts<'a> {
    pub first_letter_excluded: bool,
    pub text_shadow_ink: Option<Color>,
    pub ruby_unit: bool,
    pub ruby_suppress_breaks: bool,
    pub turned_line: bool,
    pub lines_reversed: Option<bool>,
    pub inline_format: Option<InlineFormat>,
    pub flat_mode: Option<WritingModeKw>,
    pub inline_decoration: InlineDecoration,
    pub no_select: Option<bool>,
    pub foreground: ForegroundPaint,
    pub filter: Option<Filter>,
    pub filter_ops: Cow<'a, [Filter]>,
    pub font_px: f32,
    /// A flex or grid container: its children are blockified (decorations still propagate).
    pub blockified: bool,
    pub decors: Rc<[Decor]>,
}

impl<'a> ParentFacts<'a> {
    pub fn of(parents: BoxParents<'a>, vcx: ValueCx<'_>) -> Self {
        Self {
            layout: LayoutFacts::of(parents.layout, vcx),
            tree: TreeFacts::of(parents.tree),
        }
    }

    /// The same facts without borrowing the parents: the key a stage record keeps.
    pub fn into_owned(self) -> ParentFacts<'static> {
        ParentFacts {
            layout: self.layout,
            tree: TreeFacts {
                filter_ops: Cow::Owned(self.tree.filter_ops.into_owned()),
                decors: self.tree.decors,
                first_letter_excluded: self.tree.first_letter_excluded,
                text_shadow_ink: self.tree.text_shadow_ink,
                ruby_unit: self.tree.ruby_unit,
                ruby_suppress_breaks: self.tree.ruby_suppress_breaks,
                turned_line: self.tree.turned_line,
                lines_reversed: self.tree.lines_reversed,
                inline_format: self.tree.inline_format,
                flat_mode: self.tree.flat_mode,
                inline_decoration: self.tree.inline_decoration,
                no_select: self.tree.no_select,
                foreground: self.tree.foreground,
                filter: self.tree.filter,
                font_px: self.tree.font_px,
                blockified: self.tree.blockified,
            },
        }
    }
}

impl LayoutFacts {
    fn of(parent: &BoxStyle, vcx: ValueCx<'_>) -> Self {
        let cb = establishes_cb(parent);
        let laid_out = matches!(
            parent.reset_box_layout.display,
            Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes | D::TableCell)
        );
        Self {
            context: parent.child_context(),
            cb_ancestor: parent.cb_ancestor || cb,
            cb_depth: parent.cb_depth + u16::from(cb),
            sc_ancestor: crate::style::computed::stacking::nested_context(parent),
            cb_scroll_insets: if cb {
                let scroll = &parent.reset_box_layout.scroll;
                (scroll.used, scroll.native)
            } else {
                parent.cb_scroll_insets
            },
            fragment_clip_ancestor: parent.tree.fragment_clip_ancestor || clips(parent),
            in_multicol: parent.tree.in_multicol || multicol(parent),
            backdrop_root_above: parent.tree.backdrop_root_above || backdrop_root(parent),
            transform_ancestor: parent.tree.transform_ancestor || fixed_barrier(parent),
            transformed_ancestor: parent.tree.transformed_ancestor || transformed(parent),
            orthogonal_scrollport: parent.tree.orthogonal_scrollport,
            flex_or_grid: matches!(
                parent.reset_box_layout.display,
                Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid)
            ),
            laid_out,
            rel_shift: parent.reset_positioned.rel_shift,
            box_shift: parent.reset_positioned.box_shift,
            child_cb_height_def: child_height_definite(parent, laid_out, vcx),
            quirk_pct_base: parent.quirk_pct_base,
            height: parent.reset_box_layout.height,
            ortho_limit: parent.tree.ortho_limit,
            orthogonal_inline: parent.tree.orthogonal_inline,
            line_clamp: (parent.tree.line_clamp).filter(|_| parent.clamp_block_container()),
            reversed_axes: super::flow_align::reversed_axes(parent),
        }
    }
}

impl<'a> TreeFacts<'a> {
    fn of(parent: &'a BoxStyle) -> Self {
        let tree = &parent.tree;
        Self {
            first_letter_excluded: parent.reset_text_box.first_letter_excluded,
            text_shadow_ink: tree.text_shadow_ink,
            ruby_unit: tree.ruby_unit,
            ruby_suppress_breaks: tree.ruby_suppress_breaks,
            turned_line: tree.turned_line,
            lines_reversed: parent.reset_context.lines_reversed,
            inline_format: parent.inline_format,
            flat_mode: parent.inh.writing.flat_mode,
            inline_decoration: parent.inline_decoration,
            no_select: parent.tree.no_select,
            foreground: parent.foreground_paint(),
            filter: parent.paint_filter(),
            filter_ops: Cow::Borrowed(parent.paint_filter_ops()),
            font_px: crate::style::cascade::compute::parent_font_px(parent),
            blockified: matches!(
                parent.reset_box_layout.display,
                Some(D::Flex | D::InlineFlex | D::Grid | D::InlineGrid | D::GridLanes)
            ),
            decors: parent.decors.clone(),
        }
    }

    /// The decorations a child propagates (css-text-decor-3 §2.1): none into an atomic
    /// inline or a box out of flow, unless a flex or grid parent blockified it.
    pub fn decoration_inputs(
        &self,
        own: &crate::style::computed::Computed,
    ) -> crate::box_facts::DecorationInputs<'_, Decor> {
        let blocked = matches!(own.reset_positioned.position, Some(P::Absolute | P::Fixed))
            || own.reset_box_layout.float.is_some_and(|f| f != 0)
            || (!self.blockified
                && matches!(
                    own.reset_box_layout.display,
                    Some(D::InlineBlock | D::InlineTable | D::InlineFlex | D::InlineGrid)
                ));
        crate::box_facts::DecorationInputs {
            inherited: (!blocked).then_some(&self.decors),
            block: match own.reset_box_layout.display {
                None => own.block_tag && own.reset_context.inline_display != Some(true),
                Some(D::Contents) => false,
                Some(_) => true,
            },
            generates_box: own.reset_box_layout.display != Some(D::Contents),
        }
    }
}

mod diff;
mod predicates;
pub(super) use predicates::transformed;
use predicates::{child_height_definite, clips, fixed_barrier, multicol};
