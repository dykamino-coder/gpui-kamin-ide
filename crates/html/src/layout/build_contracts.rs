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

use crate::dom::{Element, Node};
use crate::layout::cx::LayoutCx;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use gpui::AnyElement;

type BuildNodes = fn(&[Node], &Computed, &RenderOpts) -> Vec<AnyElement>;
type BuildElement = fn(&Element, &Computed, &RenderOpts) -> AnyElement;
type BuildParagraph = fn(&[Node], &Computed, &RenderOpts) -> AnyElement;
type BuildStyledBox = fn(&Element, &Computed, &LayoutCx) -> gpui::Div;

/// The build requests the algorithms need: a block-level node list
/// (`blocks`), one element box (`element`), one inline formatting context
/// (`paragraph`) and the styled, childless div of an element under a given
/// style (`styled_box`, paint's `styled_div_with`). Inputs are the inherited
/// computed style and the options or layout context of the box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    blocks: BuildNodes,
    element: BuildElement,
    paragraph: BuildParagraph,
    styled_box: BuildStyledBox,
}

impl ChildBuild {
    pub(crate) const fn new(
        blocks: BuildNodes,
        element: BuildElement,
        paragraph: BuildParagraph,
        styled_box: BuildStyledBox,
    ) -> Self {
        Self {
            blocks,
            element,
            paragraph,
            styled_box,
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
}
