//! Реализация в render контракта раскладки по сборке дочерних элементов.

use crate::layout::build_contracts::{ChildBuild, Effect, FlowBuild, Layer};
use crate::paint::decorations::outline_phase;
use crate::paint::table::{self, CellBgPainter, CellsClipped, EdgePainter};
use crate::render::{blocks::blocks, element::element, paragraph::paragraph};
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
            crate::paint::box_tree::styled::styled_div_with,
            crate::paint::replaced::image::image_with,
            effects,
            layer,
        )
    }
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
