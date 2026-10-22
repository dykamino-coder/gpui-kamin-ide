//! Контракт построения потомков: как алгоритмы layout, text и paint запрашивают
//! боксы внутри того, который они раскладывают.
//!
//! В Servo рекурсией владеет построитель дерева боксов, в Blink — обход дерева
//! layout-объектов; алгоритмы получают уже построенных потомков. Здесь обход — это
//! оркестратор render, поэтому алгоритм, которому нужно построить собственных
//! потомков (содержимое list item, ячейка таблицы, копия колонки), вызывает
//! операции этого контракта из [`LayoutCx::build`], не импортируя render. Тип
//! контракта принадлежит потребителям; реализацию даёт render (`impl Default for
//! ChildBuild` в `render/child_builder.rs`), так что её несёт каждый `LayoutCx`.
//!
//! [`LayoutCx::build`]: crate::layout::cx::LayoutCx::build

mod flow;
pub(crate) use flow::FlowBuild;
mod boxes;
pub(crate) use boxes::{BoxBuild, GridPlacement, Styling};

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::layout::table::probes::{CellBgs, CellEdges, RowRects};
use crate::paint::context::PaintCx;
use crate::style::computed::Computed;
use crate::text::clamp::ClampSpan;
use gpui::AnyElement;
use std::rc::Rc;

type BuildElement = fn(&Element, &Computed, &RenderOpts) -> AnyElement;
type BuildReplaced = fn(&Element, Option<f32>, &LayoutCx) -> AnyElement;
type BuildEffect = fn(AnyElement, Effect<'_>) -> AnyElement;
type BuildLayer = fn(Layer<'_>) -> AnyElement;

/// A paint effect around a box the algorithm has built. The box keeps its
/// layout; the effect only changes how and when it is painted.
pub(crate) enum Effect<'a> {
    /// The outline phase (CSS 2.1 Appendix E, step 10) isolated inside the
    /// box: a float is painted atomically (E.2 step 5), so its outlines are
    /// painted with it rather than with the enclosing phase.
    OutlineScope(&'a Rc<PaintCx>),
    /// The outline phase of a formatting context, which is no stacking
    /// context: the enclosing phase is reused when one is collecting.
    OutlineFlow(&'a Rc<PaintCx>),
}

/// A paint layer over geometry that layout's probes record in prepaint of
/// the same frame (`layout::table::probes`). The layer is laid out of flow
/// and only paints.
pub(crate) enum Layer<'a> {
    /// A row, row-group or column band: its background, shadows and outline
    /// painted inside the cells of the band (css-tables-3
    /// §drawing-backgrounds), under the given style.
    CellBand {
        rects: RowRects,
        style: Box<Computed>,
        vcx: ValueCx<'a>,
    },
    /// The cell backgrounds of a collapsed-border table, below its edges.
    CellBackgrounds(CellBgs),
    /// The collapsed edges of a table (CSS 2.1 §17.6.2.1).
    CollapsedEdges(CellEdges),
}

impl<'a> Layer<'a> {
    /// The band over `rects` under `style`, its values from the build `cx`.
    pub(crate) fn cell_band(rects: RowRects, style: Computed, cx: &'a LayoutCx) -> Self {
        Layer::CellBand {
            rects,
            style: Box::new(style),
            vcx: cx.vcx(),
        }
    }
}

/// The build requests the algorithms need: the normal-flow requests
/// ([`FlowBuild`]: a block-level node list, one inline formatting context,
/// the children's boundary probes and the `::first-line` routing), one
/// element box (`element`), the childless boxes styled from computed values
/// (`boxes`: [`BoxBuild`]), the box of a replaced element with an image
/// source (`replaced`, paint's `image_with`), a paint effect around a built
/// box (`effects`) and a paint layer over probed geometry (`layer`). Inputs
/// are the inherited computed style and the options or layout context of the
/// box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    flow: FlowBuild,
    element: BuildElement,
    boxes: BoxBuild,
    replaced: BuildReplaced,
    effects: BuildEffect,
    layer: BuildLayer,
}

impl ChildBuild {
    pub(crate) const fn new(
        flow: FlowBuild,
        element: BuildElement,
        boxes: BoxBuild,
        replaced: BuildReplaced,
        effects: BuildEffect,
        layer: BuildLayer,
    ) -> Self {
        Self {
            flow,
            element,
            boxes,
            replaced,
            effects,
            layer,
        }
    }

    /// Boxes of a node list in normal flow.
    pub(crate) fn blocks(
        &self,
        nodes: &[Node],
        inherited: &Computed,
        opts: &RenderOpts,
    ) -> Vec<AnyElement> {
        self.flow.blocks(nodes, inherited, opts)
    }

    /// The box of one element.
    pub(crate) fn element(
        &self,
        e: &Element,
        inherited: &Computed,
        opts: &RenderOpts,
    ) -> AnyElement {
        (self.element)(e, inherited, opts)
    }

    /// One inline formatting context over `nodes`.
    pub(crate) fn paragraph(
        &self,
        nodes: &[Node],
        inherited: &Computed,
        opts: &RenderOpts,
    ) -> AnyElement {
        self.flow.paragraph(nodes, inherited, opts)
    }

    /// The styled div of `e` under `style`, without children: its box,
    /// hover layer, scroll target and the decorations `gpui::Style` lacks.
    pub(crate) fn styled_box(&self, e: &Element, style: &Computed, cx: &LayoutCx) -> gpui::Div {
        self.boxes.styled_box(e, style, cx)
    }

    /// A div without an element, styled with the computed values `styling`.
    pub(crate) fn styled_div(&self, styling: Styling<'_>, cx: &LayoutCx) -> gpui::Div {
        self.boxes.styled_div(styling, cx)
    }

    /// `el` sized by its content (`width`/`height: min-content | max-content
    /// | fit-content` of `style`, css-sizing-3 §5.1): wrapped in a one-track
    /// grid whose track measures it. `item_style` is the item's style with
    /// inheritance applied; `placement` is its place in the parent's grid.
    pub(crate) fn content_sized(
        &self,
        el: AnyElement,
        style: &Computed,
        item_style: &Computed,
        placement: GridPlacement,
        cx: &LayoutCx,
    ) -> AnyElement {
        self.boxes
            .content_sized(el, style, item_style, placement, cx)
    }

    /// Пробы границ детей с теми же областями clamp и gap rules, что в потоке.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn probes(
        &self,
        e: &Element,
        merged: &Computed,
        opts: &RenderOpts,
        kids: &mut Vec<AnyElement>,
        makes_bfc: bool,
        is_clamp: bool,
        span: Option<ClampSpan>,
    ) {
        self.flow
            .probes(e, merged, opts, kids, makes_bfc, is_clamp, span)
    }

    /// Слой первой строки передаётся первому блочному потомку в потоке.
    pub(crate) fn first_line_route(&self, nodes: Vec<Node>, parent: &Computed) -> Vec<Node> {
        self.flow.first_line_route(nodes, parent)
    }

    /// The box of the replaced element `e` with an image source, its picture
    /// sized by layout's replaced sizing (`layout::replaced`). `base_font`
    /// resolves font-relative lengths of a raw style; `None` when they are
    /// already resolved.
    pub(crate) fn replaced(
        &self,
        e: &Element,
        base_font: Option<f32>,
        cx: &LayoutCx,
    ) -> AnyElement {
        (self.replaced)(e, base_font, cx)
    }

    /// `child` under the paint effect `effect`.
    pub(crate) fn effects(&self, child: AnyElement, effect: Effect<'_>) -> AnyElement {
        (self.effects)(child, effect)
    }

    /// The paint layer `layer`.
    pub(crate) fn layer(&self, layer: Layer<'_>) -> AnyElement {
        (self.layer)(layer)
    }
}
