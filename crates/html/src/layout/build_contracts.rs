//! Child-build contract: how layout, text and paint algorithms ask for the
//! boxes inside the one they lay out.
//!
//! In Servo the box-tree builder and in Blink the layout object tree walk
//! own the recursion; the algorithms receive built children. Here the walk is
//! the render orchestrator, so an algorithm that must build its own children
//! (a list item's contents, a table cell, a column's copy) calls the
//! operations of this contract carried in [`LayoutCx::build`] instead of
//! importing render. The contract type belongs to its consumers; render
//! supplies the implementation (`impl Default for ChildBuild` in
//! `render/child_builder.rs`), so every `LayoutCx` carries it.
//!
//! [`LayoutCx::build`]: crate::layout::cx::LayoutCx::build

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use gpui::AnyElement;

type BuildNodes = fn(&[Node], &Computed, &RenderOpts) -> Vec<AnyElement>;
type BuildElement = fn(&Element, &Computed, &RenderOpts) -> AnyElement;
type BuildParagraph = fn(&[Node], &Computed, &RenderOpts) -> AnyElement;

/// The three build requests the algorithms need: a block-level node list
/// (`blocks`), one element box (`element`) and one inline formatting context
/// (`paragraph`). Inputs are the inherited computed style and the options of
/// the box being built.
#[derive(Clone, Copy)]
pub(crate) struct ChildBuild {
    blocks: BuildNodes,
    element: BuildElement,
    paragraph: BuildParagraph,
}

impl ChildBuild {
    pub(crate) const fn new(
        blocks: BuildNodes,
        element: BuildElement,
        paragraph: BuildParagraph,
    ) -> Self {
        Self {
            blocks,
            element,
            paragraph,
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
}
