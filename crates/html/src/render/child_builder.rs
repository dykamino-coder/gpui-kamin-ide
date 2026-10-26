//! Реализация в render контракта раскладки по сборке дочерних элементов.

use crate::hooks::IntoHooked;
use crate::layout::build_contracts::{
    BoxBuild, ChildBuild, Effect, EffectBuild, FlowBuild, GridPlacement, Layer, Styling,
};
use crate::layout::cx::LayoutCx;
use crate::paint::apply;
use crate::paint::box_tree::{content_wrapper, styled::styled_div_with};
use crate::paint::decorations::outline_phase;
use crate::paint::effects::{grouped, transform, underlay::Underlay};
use crate::paint::gap_rules::painter::GapRulePainter;
use crate::paint::stacking;
use crate::paint::table::{self, CellBgPainter, CellsClipped, EdgePainter};
use crate::render::{blocks::blocks, element::element, paragraph::paragraph};
use crate::style::computed::BoxStyle;
use gpui::AnyElement;

impl Default for ChildBuild {
    fn default() -> Self {
        ChildBuild::new(
            FlowBuild::new(
                blocks,
                |stk, nodes, inherited, opts| {
                    Box::pin(crate::render::blocks::blocks_stack(
                        stk, nodes, inherited, opts,
                    ))
                },
                paragraph,
                crate::render::generic_box::child_probes,
                crate::render::first_line_descendants::route,
            ),
            element,
            |stk, e, inherited, opts| {
                Box::pin(crate::render::element::element_stack(
                    stk, e, inherited, opts,
                ))
            },
            BoxBuild::new(
                styled_div_with,
                styled_div,
                content_sized,
                content_wrapper::for_element,
                apply::len_to_gpui,
                apply::self_align,
            ),
            crate::paint::replaced::image::image_with,
            crate::svg::element_with_html,
            EffectBuild::new(
                effects,
                crate::paint::effects::grouped::paint_group,
                crate::paint::effects::mask::collect_mask_defs,
                |owner, window, cx, body| outline_phase::flow(Some(owner), window, cx, body),
            ),
            layer,
        )
    }
}

fn styled_div(styling: Styling<'_>, cx: &LayoutCx) -> gpui::Div {
    let vcx = cx.vcx();
    match styling {
        Styling::Box(style) => apply::apply(gpui::div(), style, vcx),
        Styling::Text(style) => apply::apply_text(gpui::div(), style, vcx),
        Styling::Margins(margins) => apply::margins(gpui::div(), margins, vcx),
    }
}

fn content_sized(
    el: AnyElement,
    style: &BoxStyle,
    item_style: &BoxStyle,
    placement: GridPlacement,
    cx: &LayoutCx,
) -> AnyElement {
    content_wrapper::content_sized(el, style, item_style, placement, cx.vcx())
}

fn effects(child: AnyElement, effect: Effect<'_>) -> AnyElement {
    match effect {
        Effect::OutlineScope(owner) => outline_phase::scope(child, Some(owner)),
        Effect::OutlineFlow(owner) => outline_phase::flow_phase(child, Some(owner)),
        Effect::Group { style, vcx } => grouped::grouped(child, &BoxStyle::shared(style), vcx),
        Effect::GroupReplaced { style, vcx } => {
            grouped::grouped_replaced(child, &BoxStyle::shared(style), vcx)
        }
        Effect::Transform {
            style,
            parent,
            ref_box,
            vcx,
        } => transform::transformed_with(child, style, parent, ref_box, vcx),
        Effect::Underlay => Underlay::new(child).hooked(),
        Effect::PaintLast(key) => stacking::paint_last(child, key),
    }
}

fn layer(layer: Layer<'_>) -> AnyElement {
    match layer {
        Layer::DeferredPositioned(publication) => {
            crate::paint::positioned::deferred_source(publication)
        }
        Layer::CellBand { rects, style, vcx } => {
            table::layer(CellsClipped::new(rects, *style, vcx))
        }
        Layer::CellBackgrounds(bgs) => table::layer(CellBgPainter::new(bgs)),
        Layer::CollapsedEdges(edges) => table::layer(EdgePainter::new(edges)),
        Layer::GapRules { items, spec, owner } => {
            crate::hooks::PaintOnly(GapRulePainter::new(items, *spec, owner)).hooked()
        }
    }
}
