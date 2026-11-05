//! Адаптер spanner path: native-геометрия при разрешённой ширине и расход
//! внешнего ряда. Временные LayoutId не переживают вложенный layout.

use super::super::columns::column_plan_in;
use crate::layout::multicol::dimensions::MulticolDimensions;
mod build;
mod intrinsic;
mod probe;
use crate::dom::{Element, Node};
use crate::layout::fragment::token::space::sequence::Nested;
use crate::layout::multicol::measure::{SpannerRow, row_plan_in, tail_rest_height};
use crate::layout::multicol::spanner::{
    RowMeasures, RowSlot, SpannerFlow, SpannerPass, hoist_spanners_async, spanner_box,
};
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Computed};
use build::build;
use gpui::{AnyElement, AvailableSpace, px, size};
use std::rc::Rc;

pub(super) fn deferred(
    e: &Element,
    merged: &BoxStyle,
    inherited: &Computed,
    opts: &RenderOpts,
    outer: Option<Rc<Nested>>,
) -> AnyElement {
    let mut opts = opts.clone();
    opts.cx.position.state = opts.cx.position.state.retain().resume();
    let recipe = Rc::new(Recipe {
        e: e.clone(),
        merged: merged.clone(),
        inherited: inherited.clone(),
        opts,
        outer,
        row_measures: Rc::new(RowMeasures::default()),
    });
    let intrinsic = recipe.clone();
    SpannerFlow::deferred(
        move |available, pass, window, cx, stack| {
            let recipe = recipe.clone();
            Box::pin(async move { recipe.build_async(available, pass, window, cx, stack).await })
        },
        move |available, window, cx, stack| {
            let recipe = intrinsic.clone();
            Box::pin(async move {
                intrinsic::measure_async(
                    &recipe.e,
                    &recipe.merged,
                    &recipe.opts,
                    available,
                    window,
                    cx,
                    stack,
                )
                .await
            })
        },
    )
}
fn is_span(n: &Node) -> bool {
    matches!(n,Node::Element(e) if spanner_box(e))
}
struct Recipe {
    e: Element,
    merged: BoxStyle,
    inherited: Computed,
    opts: RenderOpts,
    outer: Option<Rc<Nested>>,
    row_measures: Rc<RowMeasures>,
}
mod materialize;
