//! Fit-content clamps the requested width between independent intrinsic probes.
//! A definite available width can produce overflowing inline content below its minimum.
use super::*;
use crate::CompactLength;

/// KaminIDE patch: Measure a block width requested by an intrinsic sizing keyword.
pub(super) async fn measure_async(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    style: Dimension,
    resolution: SizingKeywordResolution,
    parent: Size<Option<f32>>,
    collapsible: Line<bool>,
    stack: &mut reblessive::Stk,
) -> f32 {
    let available = match resolution {
        SizingKeywordResolution::Exact(value) => return value,
        SizingKeywordResolution::Measure(available) => available,
    };
    let fit = matches!(
        style.tag(),
        CompactLength::FIT_CONTENT_PX_TAG
            | CompactLength::FIT_CONTENT_PERCENT_TAG
            | CompactLength::FIT_CONTENT_KEYWORD_TAG
    );
    let Some(mut argument) = available.into_option().filter(|_| fit) else {
        return probe_async(tree, node, parent, available, collapsible, stack).await;
    };
    if style.tag() != CompactLength::FIT_CONTENT_KEYWORD_TAG {
        let core = tree.get_core_container_style(node);
        if core.box_sizing() == BoxSizing::ContentBox {
            let edges = core
                .padding()
                .resolve_or_zero(parent.width, |v, basis| tree.calc(v, basis))
                + core
                    .border()
                    .resolve_or_zero(parent.width, |v, basis| tree.calc(v, basis));
            argument += edges.horizontal_axis_sum();
        }
    }
    let minimum = probe_async(
        tree,
        node,
        parent,
        AvailableSpace::MinContent,
        collapsible,
        stack,
    )
    .await;
    let maximum = probe_async(
        tree,
        node,
        parent,
        AvailableSpace::MaxContent,
        collapsible,
        stack,
    )
    .await;
    argument.max(minimum).min(maximum)
}

/// KaminIDE patch: Run an intrinsic width probe with the requested available space.
async fn probe_async(
    tree: &mut impl LayoutPartialTree,
    node: NodeId,
    parent: Size<Option<f32>>,
    width: AvailableSpace,
    collapsible: Line<bool>,
    stack: &mut reblessive::Stk,
) -> f32 {
    tree.measure_child_size_async(
        node,
        Size::NONE,
        parent,
        Size {
            width,
            height: AvailableSpace::MaxContent,
        },
        SizingMode::InherentSize,
        crate::AbsoluteAxis::Horizontal,
        collapsible,
        stack,
    )
    .await
}
