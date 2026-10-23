//! Normal-flow requests of the build contract: the boxes of a block-level
//! node list, one inline formatting context, the boundary probes of a
//! flow's children and the routing of the `::first-line` layer to the first
//! block in flow. The orchestrator in render builds them; layout asks.

use crate::dom::{Element, Node};
use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use crate::text::clamp::ClampSpan;
use gpui::AnyElement;

type BuildNodes = fn(&[Node], &BoxStyle, &RenderOpts) -> Vec<AnyElement>;
type BuildParagraph = fn(&[Node], &BoxStyle, &RenderOpts) -> AnyElement;
type BuildProbes =
    fn(&Element, &BoxStyle, &RenderOpts, &mut Vec<AnyElement>, bool, bool, Option<ClampSpan>);
type FirstLineRoute = fn(Vec<Node>, &crate::style::computed::Computed) -> Vec<Node>;

/// The normal-flow operations of [`ChildBuild`](super::ChildBuild).
#[derive(Clone, Copy)]
pub(crate) struct FlowBuild {
    blocks: BuildNodes,
    paragraph: BuildParagraph,
    probes: BuildProbes,
    first_line_route: FirstLineRoute,
}

impl FlowBuild {
    pub(crate) const fn new(
        blocks: BuildNodes,
        paragraph: BuildParagraph,
        probes: BuildProbes,
        first_line_route: FirstLineRoute,
    ) -> Self {
        Self {
            blocks,
            paragraph,
            probes,
            first_line_route,
        }
    }

    pub(super) fn blocks(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> Vec<AnyElement> {
        (self.blocks)(nodes, inherited, opts)
    }

    pub(super) fn paragraph(
        &self,
        nodes: &[Node],
        inherited: &BoxStyle,
        opts: &RenderOpts,
    ) -> AnyElement {
        (self.paragraph)(nodes, inherited, opts)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn probes(
        &self,
        e: &Element,
        merged: &BoxStyle,
        opts: &RenderOpts,
        kids: &mut Vec<AnyElement>,
        makes_bfc: bool,
        is_clamp: bool,
        span: Option<ClampSpan>,
    ) {
        (self.probes)(e, merged, opts, kids, makes_bfc, is_clamp, span)
    }

    pub(super) fn first_line_route(
        &self,
        nodes: Vec<Node>,
        parent: &crate::style::computed::Computed,
    ) -> Vec<Node> {
        (self.first_line_route)(nodes, parent)
    }
}
