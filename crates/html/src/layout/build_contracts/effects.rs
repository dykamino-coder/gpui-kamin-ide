//! Paint effects of the build contract: an effect around a box the algorithm
//! has built, the paint group of a block box around its container and the
//! document's paint resources. Layout decides which effect a box needs and
//! under which style; paint's wrappers (`paint::effects`, `paint::decorations`,
//! `paint::stacking`) apply it.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::cx::deferred::Deferred;
use crate::style::box_input::BoxStyleInput;
use crate::style::computed::{BoxStyle, Computed};
use gpui::{AnyElement, App, Bounds, Pixels, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Union of unrounded boxes, filled on prepaint (see `Transformed::ref_box`).
pub(crate) type RefBox = Rc<Cell<Option<Bounds<Pixels>>>>;

/// A paint effect around a box the algorithm has built. The box keeps its
/// layout; the effect only changes how and when it is painted.
pub(crate) enum Effect<'a> {
    /// The outline phase (CSS 2.1 Appendix E, step 10) isolated inside the
    /// box: a float is painted atomically (E.2 step 5), so its outlines are
    /// painted with it rather than with the enclosing phase.
    OutlineScope(&'a Rc<Deferred>),
    /// The outline phase of a formatting context, which is no stacking
    /// context: the enclosing phase is reused when one is collecting.
    OutlineFlow(&'a Rc<Deferred>),
    /// The grouped effects of `style` (opacity, filter, blending, isolation,
    /// clipping and masking): the box painted as one image when an effect
    /// needs the finished picture.
    Group {
        style: BoxStyleInput,
        vcx: ValueCx<'a>,
    },
    /// The group of a replaced box with shaped corners (css-borders-4
    /// §corner-shape-rendering): its content clipped by the inner contour,
    /// the border ring over it.
    GroupReplaced {
        style: BoxStyleInput,
        vcx: ValueCx<'a>,
    },
    /// The transform of `style` (css-transforms-1/2) under `parent`, with a
    /// reference box shared by the cells of a table row or row group
    /// (`ref_box`) or the box's own one (`None`).
    Transform {
        style: &'a Computed,
        parent: &'a Computed,
        ref_box: Option<RefBox>,
        vcx: ValueCx<'a>,
    },
    /// A negative `z-index` (CSS 2.1 §9.9, step 3): the box stays in its
    /// slot, its paint goes under the content painted before it.
    Underlay,
    /// CSS 2.1 Appendix E step 8: the box is painted in the stacking
    /// context's collector under the paint key `key`, after the inline
    /// content of step 7.
    PaintLast(u64),
}

impl<'a> Effect<'a> {
    /// The grouped effects of `style`.
    pub(crate) fn group(style: impl Into<BoxStyleInput>, vcx: ValueCx<'a>) -> Self {
        Effect::Group {
            style: style.into(),
            vcx,
        }
    }

    /// The transform of `style` under `parent` against the box's own
    /// reference box.
    pub(crate) fn transform(style: &'a Computed, parent: &'a Computed, vcx: ValueCx<'a>) -> Self {
        Effect::Transform {
            style,
            parent,
            ref_box: None,
            vcx,
        }
    }
}

pub(crate) type PaintBody<'a> = &'a mut dyn FnMut(&mut Window, &mut App);
type OutlineFlow = fn(&Rc<Deferred>, &mut Window, &mut App, PaintBody<'_>);

type ApplyEffect = fn(AnyElement, Effect<'_>) -> AnyElement;
type PaintGroup = fn(gpui::Div, &Element, &BoxStyle, ValueCx<'_>) -> AnyElement;
type PaintResources = fn(&[Node], ValueCx<'_>);

/// The paint operations of [`ChildBuild`](super::ChildBuild).
#[derive(Clone, Copy)]
pub(crate) struct EffectBuild {
    effect: ApplyEffect,
    paint_group: PaintGroup,
    paint_resources: PaintResources,
    outline_flow: OutlineFlow,
}

impl EffectBuild {
    pub(crate) const fn new(
        effect: ApplyEffect,
        paint_group: PaintGroup,
        paint_resources: PaintResources,
        outline_flow: OutlineFlow,
    ) -> Self {
        Self {
            effect,
            paint_group,
            paint_resources,
            outline_flow,
        }
    }

    pub(super) fn outline_flow(
        &self,
        owner: &Rc<Deferred>,
        window: &mut Window,
        cx: &mut App,
        body: PaintBody<'_>,
    ) {
        (self.outline_flow)(owner, window, cx, body)
    }

    pub(super) fn effect(&self, child: AnyElement, effect: Effect<'_>) -> AnyElement {
        (self.effect)(child, effect)
    }

    pub(super) fn paint_group(
        &self,
        container: gpui::Div,
        e: &Element,
        inherited: &BoxStyle,
        vcx: ValueCx<'_>,
    ) -> AnyElement {
        (self.paint_group)(container, e, inherited, vcx)
    }

    pub(super) fn paint_resources(&self, nodes: &[Node], vcx: ValueCx<'_>) {
        (self.paint_resources)(nodes, vcx)
    }
}
