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

mod boxes;
pub(crate) use boxes::{BoxBuild, GridPlacement, Styling};
mod effects;
pub(crate) use effects::{Effect, EffectBuild, RefBox};

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::layout::table::probes::{CellBgs, CellEdges, RowRects};
use crate::style::computed::Computed;
use gpui::AnyElement;

type BuildNodes = fn(&[Node], &Computed, &RenderOpts) -> Vec<AnyElement>;
type BuildElement = fn(&Element, &Computed, &RenderOpts) -> AnyElement;
type BuildParagraph = fn(&[Node], &Computed, &RenderOpts) -> AnyElement;
type BuildReplaced = fn(&Element, Option<f32>, &LayoutCx) -> AnyElement;
type BuildLayer = fn(Layer<'_>) -> AnyElement;

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

/// The build requests the algorithms need: a block-level node list
/// (`blocks`), one element box (`element`), one inline formatting context
/// (`paragraph`), the childless boxes styled from computed values
/// (`boxes`: [`BoxBuild`]), the box of a replaced
/// element with an image source (`replaced`, paint's `image_with`), the
/// paint effects around built boxes (`effects`: [`EffectBuild`]) and a paint
/// layer over probed geometry (`layer`).
/// Inputs are the inherited computed style and the options or layout context
/// of the box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    blocks: BuildNodes,
    element: BuildElement,
    paragraph: BuildParagraph,
    boxes: BoxBuild,
    replaced: BuildReplaced,
    effects: EffectBuild,
    layer: BuildLayer,
}

impl ChildBuild {
    pub(crate) const fn new(
        blocks: BuildNodes,
        element: BuildElement,
        paragraph: BuildParagraph,
        boxes: BoxBuild,
        replaced: BuildReplaced,
        effects: EffectBuild,
        layer: BuildLayer,
    ) -> Self {
        Self {
            blocks,
            element,
            paragraph,
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
        (self.blocks)(nodes, inherited, opts)
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
        (self.paragraph)(nodes, inherited, opts)
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
        self.effects.effect(child, effect)
    }

    /// The paint group of block `e` around `container`, a container of its
    /// border box with its descendants: the same chain as a block in the
    /// flow — the folded shift and opacity on the container, then the
    /// transform, then the grouped effects.
    pub(crate) fn paint_group(
        &self,
        container: gpui::Div,
        e: &Element,
        inherited: &Computed,
        vcx: ValueCx<'_>,
    ) -> AnyElement {
        self.effects.paint_group(container, e, inherited, vcx)
    }

    /// Prepares the paint resources of the document `nodes` (the `<mask>`,
    /// `<clipPath>` and `<filter>` definitions that `url(#id)` references)
    /// before its boxes are built: a reference may precede its definition.
    pub(crate) fn paint_resources(&self, nodes: &[Node], vcx: ValueCx<'_>) {
        self.effects.paint_resources(nodes, vcx)
    }

    /// The paint layer `layer`.
    pub(crate) fn layer(&self, layer: Layer<'_>) -> AnyElement {
        (self.layer)(layer)
    }
}
