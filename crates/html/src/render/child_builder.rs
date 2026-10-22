//! Реализация в render контракта раскладки по сборке дочерних элементов.

use crate::layout::build_contracts::{
    BoxBuild, ChildBuild, Effect, FlowBuild, GridPlacement, Layer,
};
use crate::layout::cx::LayoutCx;
use crate::paint::box_tree::{content_wrapper, styled::styled_div_with};
use crate::paint::decorations::outline_phase;
use crate::paint::table::{self, CellBgPainter, CellsClipped, EdgePainter};
use crate::render::{blocks::blocks, element::element, paragraph::paragraph};
use crate::style::computed::Computed;
use gpui::AnyElement;

impl Default for ChildBuild {
    fn default() -> Self {
        ChildBuild::new(
            FlowBuild::new(
                blocks,
                paragraph,
                crate::render::generic_box::child_probes,
                crate::render::first_line_descendants::route,
            ),
            element,
            BoxBuild::new(styled_div_with, content_sized),
            crate::paint::replaced::image::image_with,
            effects,
            layer,
        )
    }
}

fn content_sized(
    el: AnyElement,
    style: &Computed,
    item_style: &Computed,
    placement: GridPlacement,
    cx: &LayoutCx,
) -> AnyElement {
    content_wrapper::content_sized(el, style, item_style, placement, cx.vcx())
}

fn effects(child: AnyElement, effect: Effect<'_>) -> AnyElement {
    match effect {
        Effect::OutlineScope(owner) => outline_phase::scope(child, Some(owner)),
        Effect::OutlineFlow(owner) => outline_phase::flow_phase(child, Some(owner)),
    }
}

fn layer(layer: Layer<'_>) -> AnyElement {
    match layer {
        Layer::CellBand { rects, style, vcx } => {
            table::layer(CellsClipped::new(rects, *style, vcx))
        }
        Layer::CellBackgrounds(bgs) => table::layer(CellBgPainter::new(bgs)),
        Layer::CollapsedEdges(edges) => table::layer(EdgePainter::new(edges)),
    }
}
