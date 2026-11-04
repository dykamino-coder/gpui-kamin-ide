//! Resolve calc-size calculations in the native intrinsic constraint space.

use super::*;
use crate::geometry::AbsoluteAxis;
use crate::util::ResolveOrZero;
use crate::{CompactLength, Dimension, RequestedAxis, SizingMode};

impl<NodeContext, MeasureFunction> TaffyView<'_, NodeContext, MeasureFunction>
where
    MeasureFunction: crate::tree::LayoutMeasurer<NodeContext>,
{
    /// KaminIDE patch: Resolve calc-size expressions using intrinsic measurement when required.
    #[inline]
    pub(super) async fn resolve_calc_size_async(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        stack: &mut reblessive::Stk,
    ) -> LayoutInput {
        let style = &self.taffy.nodes[node.into()].style;
        if style
            .calc_size
            .iter()
            .chain(&style.calc_size_max)
            .all(Option::is_none)
        {
            return inputs;
        }
        self.resolve_calc_size_expressions_async(node, inputs, stack)
            .await
    }

    // KaminIDE patch: the resolution temporaries must not live in the native
    // frame of `compute_child_layout`, which waits for the recursive layout of
    // every descendant (a 2052-level box tree on the 8 MiB stack).
    /// Resolves the calc-size expressions of a node that has at least one.
    #[inline(never)]
    async fn resolve_calc_size_expressions_async(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        stack: &mut reblessive::Stk,
    ) -> LayoutInput {
        let style = &self.taffy.nodes[node.into()].style;
        let [w, h, min_w, min_h] = style.calc_size;
        let [max_w, max_h] = style.calc_size_max;
        let expressions = [w, h, min_w, min_h, max_w, max_h];
        let bases = style.calc_size_basis;
        let edges = if style.calc_size_content_box {
            (style
                .padding
                .resolve_or_zero(inputs.parent_size.width, calc_value)
                + style
                    .border
                    .resolve_or_zero(inputs.parent_size.width, calc_value))
            .sum_axes()
        } else {
            Size::ZERO
        };
        let ratio = style.aspect_ratio.filter(|r| *r > 0.0);
        #[cfg(not(feature = "flexbox"))]
        let flex_main: Option<usize> = None;
        #[cfg(feature = "flexbox")]
        let flex_main = self.taffy.parents[node.into()].and_then(|parent| {
            let style = &self.taffy.nodes[parent.into()].style;
            (style.display == Display::Flex).then_some(if style.flex_direction.is_row() {
                0
            } else {
                1
            })
        });
        let mut resolved = inputs;
        for (axis, physical) in [AbsoluteAxis::Horizontal, AbsoluteAxis::Vertical]
            .into_iter()
            .enumerate()
        {
            let known = inputs.known_dimensions.get_abs(physical);
            let preferred = match (known, expressions[axis]) {
                (None, Some(expression)) => {
                    let mut base = self
                        .calc_basis_async(node, inputs, physical, bases[axis], false, stack)
                        .await;
                    if bases[axis].tag() == CompactLength::AUTO_TAG {
                        if let (Some(ratio), Some(cross)) = (
                            ratio,
                            inputs.known_dimensions.get_abs(physical.other_axis()),
                        ) {
                            base = if physical == AbsoluteAxis::Horizontal {
                                cross * ratio
                            } else {
                                cross / ratio
                            };
                        }
                    }
                    Some(crate::style::apply_calc_size_box(
                        expression,
                        base,
                        edges.get_abs(physical),
                    ))
                }
                _ => known,
            };
            let mut limits = [None; 2];
            for (kind, limit) in limits.iter_mut().enumerate() {
                let index = axis + 2 + kind * 2;
                // Flexbox already resolves its automatic main-axis minimum,
                // including the specified/transferred size suggestion caps.
                if kind == 0 && flex_main == Some(axis) && bases[index].is_auto() {
                    continue;
                }
                if let Some(expression) = expressions[index] {
                    let base = self
                        .calc_basis_async(node, inputs, physical, bases[index], kind == 0, stack)
                        .await;
                    *limit = Some(crate::style::apply_calc_size_box(
                        expression,
                        base,
                        edges.get_abs(physical),
                    ));
                }
            }
            if preferred.is_none() && limits.iter().all(Option::is_none) {
                continue;
            }
            if expressions[axis].is_none() && limits.iter().all(Option::is_none) {
                continue;
            }
            let transferred = ratio
                .zip(inputs.known_dimensions.get_abs(physical.other_axis()))
                .map(|(ratio, cross)| {
                    if physical == AbsoluteAxis::Horizontal {
                        cross * ratio
                    } else {
                        cross / ratio
                    }
                });
            let value = crate::traversal::unwrap_or_else(preferred.or(transferred), async || {
                self.calc_basis_async(node, inputs, physical, Dimension::auto(), false, stack)
                    .await
            })
            .await;
            let value = value
                .min(limits[1].unwrap_or(f32::MAX))
                .max(limits[0].unwrap_or(0.0));
            resolved.known_dimensions.set_abs(physical, Some(value));
            if expressions[axis].is_some() && known.is_none() {
                resolved
                    .known_dimensions_are_definite
                    .set_abs(physical, false);
            }
        }
        resolved
    }

    /// KaminIDE patch: Measure the intrinsic basis of a calc-size expression.
    async fn calc_basis_async(
        &mut self,
        node: NodeId,
        inputs: LayoutInput,
        axis: AbsoluteAxis,
        basis: Dimension,
        minimum: bool,
        stack: &mut reblessive::Stk,
    ) -> f32 {
        let available = inputs.available_space.get_abs(axis);
        match basis.tag() {
            CompactLength::MIN_CONTENT_TAG => {
                self.calc_probe_async(node, inputs, axis, AvailableSpace::MinContent, true, stack)
                    .await
            }
            CompactLength::MAX_CONTENT_TAG => {
                self.calc_probe_async(node, inputs, axis, AvailableSpace::MaxContent, true, stack)
                    .await
            }
            CompactLength::STRETCH_TAG | CompactLength::FIT_CONTENT_KEYWORD_TAG => {
                match available {
                    AvailableSpace::Definite(value) => {
                        // LayoutInput available space is already the node's
                        // border-box opportunity, with margins subtracted.
                        let available = value.max(0.0);
                        if basis.tag() == CompactLength::STRETCH_TAG {
                            return available;
                        }
                        let min = self
                            .calc_probe_async(
                                node,
                                inputs,
                                axis,
                                AvailableSpace::MinContent,
                                true,
                                stack,
                            )
                            .await;
                        let max = self
                            .calc_probe_async(
                                node,
                                inputs,
                                axis,
                                AvailableSpace::MaxContent,
                                true,
                                stack,
                            )
                            .await;
                        available.min(max).max(min)
                    }
                    _ => {
                        self.calc_probe_async(node, inputs, axis, available, true, stack)
                            .await
                    }
                }
            }
            _ if minimum => {
                let style = &self.taffy.nodes[node.into()].style;
                let overflow = if axis == AbsoluteAxis::Horizontal {
                    style.overflow.x
                } else {
                    style.overflow.y
                };
                if style.aspect_ratio.is_some()
                    && style.size.get_abs(axis).is_auto()
                    && !style.item_is_replaced
                    && !overflow.is_scroll_container()
                {
                    self.calc_probe_async(
                        node,
                        inputs,
                        axis,
                        AvailableSpace::MinContent,
                        true,
                        stack,
                    )
                    .await
                } else {
                    0.0
                }
            }
            // Intrinsic contributions must resolve auto separately under each
            // constraint. In particular, min-content must never reuse max-content.
            _ => {
                self.calc_probe_async(
                    node,
                    inputs,
                    axis,
                    available,
                    !matches!(available, AvailableSpace::Definite(_)),
                    stack,
                )
                .await
            }
        }
    }

    /// KaminIDE patch: Run a layout probe to resolve a calc-size basis.
    async fn calc_probe_async(
        &mut self,
        node: NodeId,
        mut inputs: LayoutInput,
        axis: AbsoluteAxis,
        available: AvailableSpace,
        intrinsic: bool,
        stack: &mut reblessive::Stk,
    ) -> f32 {
        inputs.run_mode = RunMode::ComputeSize;
        inputs.axis = if axis == AbsoluteAxis::Horizontal {
            RequestedAxis::Horizontal
        } else {
            RequestedAxis::Vertical
        };
        if intrinsic {
            inputs.sizing_mode = SizingMode::IntrinsicSize;
            // A transferred preferred ratio size must not cap an intrinsic
            // minimum (CSS Sizing 4 §4.3).
            inputs.known_dimensions = Size::NONE;
        }
        inputs.known_dimensions.set_abs(axis, None);
        inputs.available_space.set_abs(axis, available);
        let measured = self
            .compute_child_layout_inner_async(
                node,
                inputs,
                #[cfg(feature = "block_layout")]
                None,
                stack,
            )
            .await;
        measured.size.get_abs(axis)
    }
}

#[cfg(test)]
#[path = "calc_size_tests.rs"]
mod tests;
