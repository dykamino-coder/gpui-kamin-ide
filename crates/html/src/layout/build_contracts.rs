//! Контракт построения потомков: как алгоритмы layout, text и paint запрашивают
//! боксы внутри того, который они раскладывают.
//!
//! В Servo рекурсией владеет построитель дерева боксов, в Blink — обход дерева
//! layout-объектов; алгоритмы получают уже построенных потомков. Здесь обход — это
//! оркестратор render, поэтому алгоритм, которому нужно построить собственных
//! потомков (содержимое list item, ячейка таблицы, копия колонки), вызывает
//! операции этого контракта из [`BuildContext::build`], не импортируя render. Тип
//! контракта принадлежит потребителям; реализацию даёт render (`impl Default for
//! ChildBuild` в `render/child_builder.rs`), так что её несёт каждый `LayoutCx`.
//!
//! [`BuildContext::build`]: crate::layout::cx::BuildContext::build

mod adapters;
mod flow;
pub(crate) use flow::FlowBuild;
mod boxes;
pub(crate) use boxes::{BoxBuild, GridPlacement, Styling};
mod layers;
pub(crate) use layers::Layer;
mod effects;
pub(crate) use effects::{Effect, EffectBuild, RefBox};

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::clamp::ClampSpan;
use gpui::AnyElement;

type BuildElement = fn(&Element, &BoxStyle, &RenderOpts) -> AnyElement;
type BuildStackElement =
    for<'a> fn(
        &'a mut reblessive::Stk,
        &'a Element,
        &'a BoxStyle,
        &'a RenderOpts,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AnyElement> + 'a>>;

type BuildReplaced = fn(&Element, Option<f32>, &LayoutCx) -> AnyElement;
type BuildLayer = fn(Layer<'_>) -> AnyElement;

/// The build requests the algorithms need: the normal-flow requests
/// ([`FlowBuild`]: a block-level node list, one inline formatting context,
/// the children's boundary probes and the `::first-line` routing), one
/// element box (`element`), the childless boxes styled from computed values
/// (`boxes`: [`BoxBuild`]), the box of a replaced element with an image
/// source (`replaced`, paint's `image_with`), the paint effects around built
/// boxes (`effects`: [`EffectBuild`]) and a paint layer over probed geometry
/// (`layer`). Inputs are the inherited computed style and the options or
/// layout context of the box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    flow: FlowBuild,
    element: BuildElement,
    element_stack: BuildStackElement,
    boxes: BoxBuild,
    replaced: BuildReplaced,
    control: fn(&Element, &BoxStyle, &RenderOpts) -> Option<AnyElement>,
    svg: fn(&Element, &RenderOpts) -> Option<AnyElement>,
    effects: EffectBuild,
    layer: BuildLayer,
}

impl ChildBuild {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        flow: FlowBuild,
        element: BuildElement,
        element_stack: BuildStackElement,
        boxes: BoxBuild,
        replaced: BuildReplaced,
        control: fn(&Element, &BoxStyle, &RenderOpts) -> Option<AnyElement>,
        svg: fn(&Element, &RenderOpts) -> Option<AnyElement>,
        effects: EffectBuild,
        layer: BuildLayer,
    ) -> Self {
        Self {
            flow,
            element,
            element_stack,
            boxes,
            replaced,
            control,
            svg,
            effects,
            layer,
        }
    }

    /// Boxes of a node list in normal flow.
    pub(crate) fn blocks(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Vec<AnyElement> {
        self.flow.blocks(nodes, inherited, opts)
    }

    /// Продолжить блочный поток на явном стеке построителя.
    pub(crate) async fn blocks_stack(
        &self,
        stk: &mut reblessive::Stk,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Vec<AnyElement> {
        self.flow.blocks_stack(stk, nodes, inherited, opts).await
    }

    /// The box of one element.
    pub(crate) fn element(
        &self,
        e: &Element,
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> AnyElement {
        (self.element)(e, inherited, opts)
    }

    /// Продолжить обход на явном стеке построителя.
    pub(crate) async fn element_stack(
        &self,
        stk: &mut reblessive::Stk,
        e: &Element,
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> AnyElement {
        stk.run(|stk| (self.element_stack)(stk, e, inherited, opts))
            .await
    }

    /// One inline formatting context over `nodes`.
    pub(crate) fn paragraph(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> AnyElement {
        self.flow.paragraph(nodes, inherited, opts)
    }

    /// The styled div of `e` under `style`, without children: its box,
    /// hover layer, scroll target and the decorations `gpui::Style` lacks.
    pub(crate) fn styled_box(&self, e: &Element, style: &BoxStyle, cx: &LayoutCx) -> gpui::Div {
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
        style: &BoxStyle,
        item_style: &BoxStyle,
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
        merged: &BoxStyle,
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
    pub(crate) fn first_line_route(
        &self,
        nodes: Vec<Node>,
        parent: &crate::style::computed::Computed,
    ) -> Vec<Node> {
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
        inherited: &BoxStyle,
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
