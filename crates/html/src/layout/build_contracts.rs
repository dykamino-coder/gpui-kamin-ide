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

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::layout::table::probes::{CellBgs, CellEdges, RowRects};
use crate::paint::context::PaintCx;
use crate::style::computed::Computed;
use gpui::AnyElement;
use std::rc::Rc;

type BuildNodes = fn(&[Node], &Computed, &RenderOpts) -> Vec<AnyElement>;
type BuildElement = fn(&Element, &Computed, &RenderOpts) -> AnyElement;
type BuildParagraph = fn(&[Node], &Computed, &RenderOpts) -> AnyElement;
type BuildStyledBox = fn(&Element, &Computed, &LayoutCx) -> gpui::Div;
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

/// The build requests the algorithms need: a block-level node list
/// (`blocks`), one element box (`element`), one inline formatting context
/// (`paragraph`), the styled, childless div of an element under a given
/// style (`styled_box`, paint's `styled_div_with`), the box of a replaced
/// element with an image source (`replaced`, paint's `image_with`), a paint
/// effect around a built box (`effects`) and a paint layer over probed
/// geometry (`layer`).
/// Inputs are the inherited computed style and the options or layout context
/// of the box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    blocks: BuildNodes,
    element: BuildElement,
    paragraph: BuildParagraph,
    styled_box: BuildStyledBox,
    replaced: BuildReplaced,
    effects: BuildEffect,
    layer: BuildLayer,
}

impl ChildBuild {
    pub(crate) const fn new(
        blocks: BuildNodes,
        element: BuildElement,
        paragraph: BuildParagraph,
        styled_box: BuildStyledBox,
        replaced: BuildReplaced,
        effects: BuildEffect,
        layer: BuildLayer,
    ) -> Self {
        Self {
            blocks,
            element,
            paragraph,
            styled_box,
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
    /// The styled div of `e` under `style`, without children.
    pub(crate) fn styled_box(&self, e: &Element, style: &Computed, cx: &LayoutCx) -> gpui::Div {
        (self.styled_box)(e, style, cx)
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
