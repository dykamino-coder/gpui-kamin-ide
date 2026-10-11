//! Alignment of tracks and final positioning of items
use super::types::GridTrack;
use crate::compute::common::alignment::{
    apply_alignment_fallback, compute_alignment_offset, resolve_self_alignment_safety,
};
use crate::geometry::{InBothAbsAxis, Line, Point, Rect, Size};
use crate::style::{
    AlignContent, AlignItems, AlignItemsKeyword, AlignSelf, AvailableSpace, CoreStyle,
    GridItemStyle, Overflow, Position,
};
use crate::tree::{Layout, LayoutPartialTreeExt, NodeId, OofCandidates, SizingMode};
use crate::util::sys::f32_max;
use crate::util::{MaybeMath, MaybeResolve, ResolveOrZero};

#[cfg(feature = "content_size")]
use crate::compute::common::scrollable_overflow::compute_scrollable_overflow_contribution;
use crate::compute::common::sizing_keyword::{resolve_sizing_keyword, SizingKeywordResolution};
use crate::{AbsoluteAxis, BoxSizing, Direction, LayoutGridContainer};

/// Align the grid tracks within the grid according to the align-content (rows) or
/// justify-content (columns) property. This only does anything if the size of the
/// grid is not equal to the size of the grid container in the axis being aligned.
pub(super) fn align_tracks(
    grid_container_content_box_size: f32,
    padding: Line<f32>,
    border: Line<f32>,
    tracks: &mut [GridTrack],
    track_alignment_style: AlignContent,
    axis_is_reversed: bool,
) {
    let used_size: f32 = tracks.iter().map(|track| track.base_size).sum();
    let free_space = grid_container_content_box_size - used_size;
    let origin = padding.start + border.start;

    // Count the number of non-collapsed tracks (not counting gutters)
    let num_tracks = tracks
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|track| !track.is_collapsed)
        .count();

    // Grid layout treats gaps as full tracks rather than applying them at alignment so we
    // simply pass zero here. Grid layout is never reversed.
    let gap = 0.0;
    let layout_is_reversed = false;
    let track_alignment = apply_alignment_fallback(free_space, num_tracks, track_alignment_style);
    let track_alignment = if axis_is_reversed {
        track_alignment.reversed()
    } else {
        track_alignment
    };

    // If every track is collapsed then no track receives the alignment offset below, but the
    // grid's lines should still be aligned within the container (e.g. at the inline-start edge
    // for RTL), so apply the offset to the origin instead.
    let empty_grid_offset = if num_tracks == 0 {
        compute_alignment_offset(
            free_space,
            num_tracks,
            gap,
            track_alignment,
            layout_is_reversed,
            true,
        )
    } else {
        0.0
    };

    // Compute offsets. Tracks are stored in logical order; when the axis is reversed (RTL)
    // physical offsets are assigned right-to-left by iterating the tracks in reverse.
    let mut total_offset = origin + empty_grid_offset;
    let mut seen_non_collapsed_track = false;
    let mut position_track = |i: usize, track: &mut GridTrack| {
        // Odd tracks are gutters (but slices are zero-indexed, so odd tracks have even indices)
        let is_gutter = i % 2 == 0;
        let is_non_collapsed_track = !is_gutter && !track.is_collapsed;

        // Alignment offsets should be applied only to non-collapsed tracks.
        let is_first = is_non_collapsed_track && !seen_non_collapsed_track;

        let offset = if is_non_collapsed_track {
            compute_alignment_offset(
                free_space,
                num_tracks,
                gap,
                track_alignment,
                layout_is_reversed,
                is_first,
            )
        } else {
            0.0
        };

        track.offset = total_offset + offset;
        total_offset = total_offset + offset + track.base_size;
        if is_non_collapsed_track {
            seen_non_collapsed_track = true;
        }
    };
    if axis_is_reversed {
        tracks
            .iter_mut()
            .rev()
            .enumerate()
            .for_each(|(i, track)| position_track(i, track));
    } else {
        tracks
            .iter_mut()
            .enumerate()
            .for_each(|(i, track)| position_track(i, track));
    }
}

/// Align and size a grid item into it's final position
#[allow(clippy::too_many_arguments)]
pub(super) fn align_and_position_item(
    tree: &mut impl LayoutGridContainer,
    node: NodeId,
    order: u32,
    grid_area: Rect<f32>,
    // Percentage basis for the item's styles. Grid passes the grid area size; grid lanes (css-grid-3) items
    // have the container's content box as their containing block in the stacking axis.
    containing_block_size: Size<f32>,
    container_alignment_styles: InBothAbsAxis<AlignItems>,
    shims: Rect<f32>,
    x_end: bool,
    margin_trim: u8,
    direction: Direction,
    container_border_box_width: f32,
    container_border: Rect<f32>,
    #[cfg(feature = "content_size")] container_is_scroll_container: bool,
    bubbled_candidates: &mut OofCandidates,
) -> (Rect<f32>, f32, f32, Option<f32>, Option<f32>) {
    let grid_area_size = Size {
        width: grid_area.right - grid_area.left,
        height: grid_area.bottom - grid_area.top,
    };

    let style = tree.get_grid_child_style(node);

    let overflow = style.overflow();
    #[cfg(feature = "content_size")]
    let contain = style.contain();
    let scrollbar_width = style.scrollbar_width();
    let aspect_ratio = style.aspect_ratio();
    let is_replaced = style.is_replaced();
    // Fall back to the container's justify-items/align-items if unset (`None`), and then resolve
    // writing-mode-relative self-start/self-end keywords against the item's own direction.
    // The horizontal axis is the inline axis (Taffy only supports horizontal-tb); the vertical
    // (block) axis resolves them to plain start/end.
    let item_direction = style.direction();
    let justify_self = style
        .justify_self()
        .unwrap_or(container_alignment_styles.horizontal)
        .resolve_self_relative(item_direction, direction, true);
    let align_self = style
        .align_self()
        .unwrap_or(container_alignment_styles.vertical)
        .resolve_self_relative(item_direction, direction, false);

    let position = style.position();
    let inset_horizontal = style.inset().horizontal_components().map(|size| {
        size.resolve_to_option(containing_block_size.width, |val, basis| {
            tree.calc(val, basis)
        })
    });
    let inset_vertical = style.inset().vertical_components().map(|size| {
        size.resolve_to_option(containing_block_size.height, |val, basis| {
            tree.calc(val, basis)
        })
    });
    let padding = style.padding().map(|p| {
        p.resolve_or_zero(Some(containing_block_size.width), |val, basis| {
            tree.calc(val, basis)
        })
    });
    let border = style.border().map(|p| {
        p.resolve_or_zero(Some(containing_block_size.width), |val, basis| {
            tree.calc(val, basis)
        })
    });
    let padding_border_size = (padding + border).sum_axes();

    let box_sizing_adjustment = if style.box_sizing() == BoxSizing::ContentBox {
        padding_border_size
    } else {
        Size::ZERO
    };

    // Linked axes ignore all authored size constraints, including native sizing keywords.
    let subgridded = if !position.is_out_of_flow() && style.grid_lanes().is_none() {
        style.subgrid() & 3
    } else {
        0
    };
    let mut size_style = style.size();
    if subgridded & 1 != 0 {
        size_style.width = crate::style::Dimension::auto();
    }
    if subgridded & 2 != 0 {
        size_style.height = crate::style::Dimension::auto();
    }
    // KaminIDE patch: явное `stretch` (не `normal`) делает ось определённой
    // без соотношения сторон (css-grid-2 §6.2; Blink kStretchExplicit против
    // kStretchImplicit). Если обе оси определены так или длиной из стиля,
    // соотношение не действует вовсе (`grid-aspect-ratio-032..037`); если
    // явно растянута только блочная, строчная при `normal` не тянется, а
    // выводится из соотношения (`grid-aspect-ratio-028/029`).
    let raw_margin = style.margin();
    // Upstream #1254: `normal` is a first-class keyword, so an explicit `stretch` is told apart
    // from `normal` by the resolved keyword (the container fallback is already applied above).
    let style_size =
        size_style.maybe_resolve(containing_block_size, |val, basis| tree.calc(val, basis));
    let stretched_w = subgridded & 1 != 0
        || (!position.is_out_of_flow()
            && justify_self.keyword == AlignItemsKeyword::Stretch
            && !raw_margin.left.is_auto()
            && !raw_margin.right.is_auto());
    let stretched_h = subgridded & 2 != 0
        || (!position.is_out_of_flow()
            && align_self.keyword == AlignItemsKeyword::Stretch
            && !raw_margin.top.is_auto()
            && !raw_margin.bottom.is_auto());
    let aspect_ratio = if (style_size.width.is_some() || stretched_w)
        && (style_size.height.is_some() || stretched_h)
    {
        None
    } else {
        aspect_ratio
    };

    let inherent_size = size_style
        .maybe_resolve(containing_block_size, |val, basis| tree.calc(val, basis))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);
    let min_size = style
        .min_size()
        .maybe_resolve(containing_block_size, |val, basis| tree.calc(val, basis))
        .maybe_add(box_sizing_adjustment)
        .or(padding_border_size.map(Some))
        .maybe_max(padding_border_size)
        .maybe_apply_aspect_ratio(aspect_ratio);
    let max_size = style
        .max_size()
        .maybe_resolve(containing_block_size, |val, basis| tree.calc(val, basis))
        .maybe_apply_aspect_ratio(aspect_ratio)
        .maybe_add(box_sizing_adjustment);

    // KaminIDE patch: css-grid-2 §subgrid-box-alignment — «The subgrid is
    // always stretched in its subgridded dimension(s): the
    // align-self/justify-self properties on it are ignored, as are any
    // specified width/height constraints». Иначе линии подсетки не совпали бы
    // с линиями родителя, по которым ей выданы дорожки. Лунки-подсетка в
    // дорожки сетки не связывается (`subgrid::linked_axes`).
    let (mut inherent_size, mut min_size, mut max_size) = (inherent_size, min_size, max_size);
    let justify_self = if subgridded & 1 != 0 {
        AlignSelf::STRETCH
    } else {
        justify_self
    };
    let align_self = if subgridded & 2 != 0 {
        AlignSelf::STRETCH
    } else {
        align_self
    };
    if subgridded & 1 != 0 {
        inherent_size.width = None;
        min_size.width = Some(padding_border_size.width);
        max_size.width = None;
    }
    if subgridded & 2 != 0 {
        inherent_size.height = None;
        min_size.height = Some(padding_border_size.height);
        max_size.height = None;
    }

    // Resolve `normal` alignment (the default if alignment is set on neither the parent or the node itself)
    // Note: if the child has a preferred aspect ratio but neither width or height are set, then the width is stretched
    // and the then height is calculated from the width according the aspect ratio
    // Replaced elements are not stretched: they are sized as for `start`.
    // See: https://www.w3.org/TR/css-grid-1/#grid-item-sizing
    let mut alignment_styles = InBothAbsAxis {
        horizontal: match justify_self.keyword {
            AlignItemsKeyword::Normal => {
                if inherent_size.width.is_some()
                    || size_style.width.is_sizing_keyword()
                    || (aspect_ratio.is_some() && stretched_h)
                    || is_replaced
                {
                    AlignSelf::START
                } else {
                    AlignSelf::STRETCH
                }
            }
            _ => justify_self,
        },
        vertical: match align_self.keyword {
            AlignItemsKeyword::Normal => {
                if inherent_size.height.is_some()
                    || size_style.height.is_sizing_keyword()
                    || aspect_ratio.is_some()
                    || is_replaced
                {
                    AlignSelf::START
                } else {
                    AlignSelf::STRETCH
                }
            }
            _ => align_self,
        },
    };

    // Note: This is not a bug. It is part of the CSS spec that both horizontal and vertical margins
    // resolve against the WIDTH of the grid area.
    let mut margin = style.margin().map(|margin| {
        margin.resolve_to_option(containing_block_size.width, |val, basis| {
            tree.calc(val, basis)
        })
    });

    // KaminIDE patch: подсеточная ось — растяжка; `auto`-поле растяжку не
    // отменяет (выравнивание подсетки игнорируется целиком).
    if subgridded & 1 != 0 {
        alignment_styles.horizontal = AlignSelf::STRETCH;
        margin.left = margin.left.or(Some(0.0));
        margin.right = margin.right.or(Some(0.0));
    }
    if subgridded & 2 != 0 {
        alignment_styles.vertical = AlignSelf::STRETCH;
        margin.top = margin.top.or(Some(0.0));
        margin.bottom = margin.bottom.or(Some(0.0));
    }
    // KaminIDE patch: `margin-trim` контейнера — обрезанное поле ноль, а не
    // авторское и не `auto` (биты сторон считает `compute_grid_layout`;
    // размеры дорожек их уже учли через `GridItem::margin`).
    if margin_trim & 1 != 0 {
        margin.top = Some(0.0);
    }
    if margin_trim & 2 != 0 {
        margin.right = Some(0.0);
    }
    if margin_trim & 4 != 0 {
        margin.bottom = Some(0.0);
    }
    if margin_trim & 8 != 0 {
        margin.left = Some(0.0);
    }

    drop(style);

    // Upstream #1234: the border-box space handed to the child as its available space.
    let available_border_box = Size {
        width: grid_area_size
            .width
            .maybe_sub(margin.left)
            .maybe_sub(margin.right)
            .max(0.0),
        height: grid_area_size
            .height
            .maybe_sub(margin.top)
            .maybe_sub(margin.bottom)
            .max(0.0),
    };
    let grid_area_minus_item_margins_size = Size {
        width: (grid_area_size
            .width
            .maybe_sub(margin.left)
            .maybe_sub(margin.right)
            - shims.left
            - shims.right)
            .max(0.0),
        height: (grid_area_size
            .height
            .maybe_sub(margin.top)
            .maybe_sub(margin.bottom)
            - shims.top
            - shims.bottom)
            .max(0.0),
    };

    // A size that is a sizing keyword (min-content, max-content, fit-content,
    // fit-content(...), stretch) either resolves to an exact size or is resolved
    // by measuring the item under the corresponding available space constraint
    let keyword_width = inherent_size.width.is_none().then(|| {
        resolve_sizing_keyword(
            size_style.width,
            Some(grid_area_minus_item_margins_size.width),
            Some(containing_block_size.width),
            |val, basis| tree.calc(val, basis),
        )
    });
    let keyword_height = inherent_size.height.is_none().then(|| {
        resolve_sizing_keyword(
            size_style.height,
            Some(grid_area_minus_item_margins_size.height),
            Some(containing_block_size.height),
            |val, basis| tree.calc(val, basis),
        )
    });

    // If both axes need to be measured then resolve them with a single measure call
    let keyword_measured_size: Size<Option<f32>> = match (&keyword_width, &keyword_height) {
        (
            Some(Some(SizingKeywordResolution::Measure(available_width))),
            Some(Some(SizingKeywordResolution::Measure(available_height))),
        ) if !position.is_out_of_flow() => tree
            .measure_child_size_both(
                node,
                Size::NONE,
                containing_block_size.map(Option::Some),
                Size {
                    width: *available_width,
                    height: *available_height,
                },
                SizingMode::InherentSize,
                Line::FALSE,
            )
            .map(Option::Some),
        _ => Size::NONE,
    };

    // If node is absolutely positioned and width is not set explicitly, then deduce it
    // from left, right and container_content_box if both are set.
    let width = inherent_size.width.or_else(|| {
        // Apply width derived from both the left and right properties of an absolutely
        // positioned element being set
        if position.is_out_of_flow() {
            if let (Some(left), Some(right)) = (inset_horizontal.start, inset_horizontal.end) {
                return Some(f32_max(
                    grid_area_minus_item_margins_size.width - left - right,
                    0.0,
                ));
            }
        }

        if let Some(Some(resolution)) = keyword_width {
            return Some(match resolution {
                SizingKeywordResolution::Exact(width) => width,
                SizingKeywordResolution::Measure(available_width) => {
                    keyword_measured_size.width.unwrap_or_else(|| {
                        tree.measure_child_size(
                            node,
                            Size::NONE,
                            containing_block_size.map(Option::Some),
                            Size {
                                width: available_width,
                                height: AvailableSpace::Definite(available_border_box.height),
                            },
                            SizingMode::InherentSize,
                            AbsoluteAxis::Horizontal,
                            Line::FALSE,
                        )
                    })
                }
            });
        }

        // Apply width based on stretch alignment if:
        //  - Alignment style is "stretch"
        //  - The node is not absolutely positioned
        //  - The node does not have auto margins in this axis.
        if margin.left.is_some()
            && margin.right.is_some()
            && alignment_styles.horizontal == AlignSelf::STRETCH
            && !position.is_out_of_flow()
        {
            return Some(grid_area_minus_item_margins_size.width);
        }

        None
    });

    // Reapply aspect ratio after stretch and absolute position width adjustments
    let Size { width, height } = Size {
        width,
        height: inherent_size.height,
    }
    .maybe_apply_aspect_ratio(aspect_ratio);

    let height = height.or_else(|| {
        if position.is_out_of_flow() {
            if let (Some(top), Some(bottom)) = (inset_vertical.start, inset_vertical.end) {
                return Some(f32_max(
                    grid_area_minus_item_margins_size.height - top - bottom,
                    0.0,
                ));
            }
        }

        if let Some(Some(resolution)) = keyword_height {
            return Some(match resolution {
                SizingKeywordResolution::Exact(height) => height,
                SizingKeywordResolution::Measure(available_height) => {
                    keyword_measured_size.height.unwrap_or_else(|| {
                        tree.measure_child_size(
                            node,
                            Size {
                                width,
                                height: None,
                            },
                            containing_block_size.map(Option::Some),
                            Size {
                                width: width.map(AvailableSpace::Definite).unwrap_or(
                                    AvailableSpace::Definite(
                                        grid_area_minus_item_margins_size.width,
                                    ),
                                ),
                                height: available_height,
                            },
                            SizingMode::InherentSize,
                            AbsoluteAxis::Vertical,
                            Line::FALSE,
                        )
                    })
                }
            });
        }

        // Apply height based on stretch alignment if:
        //  - Alignment style is "stretch"
        //  - The node is not absolutely positioned
        //  - The node does not have auto margins in this axis.
        if margin.top.is_some()
            && margin.bottom.is_some()
            && alignment_styles.vertical == AlignSelf::STRETCH
            && !position.is_out_of_flow()
        {
            return Some(grid_area_minus_item_margins_size.height);
        }

        None
    });
    // Reapply aspect ratio after stretch and absolute position height adjustments
    let Size { width, height } = Size { width, height }.maybe_apply_aspect_ratio(aspect_ratio);

    // CSS Grid §6.2 and CSS Position §4.1 use fit-content for auto widths. A definite
    // available-space probe is not this clamp: intrinsic grid/flex children can
    // still return max-content. Resolve both bounds and the actual grid area.
    let width = if width.is_none() {
        let min_content = tree.measure_child_size(
            node,
            Size {
                width: None,
                height,
            },
            containing_block_size.map(Option::Some),
            Size {
                width: AvailableSpace::MinContent,
                height: AvailableSpace::Definite(available_border_box.height),
            },
            SizingMode::InherentSize,
            AbsoluteAxis::Horizontal,
            Line::FALSE,
        );
        // The floor never exceeds max-content: taffy's min-content of a
        // wrapping column flexbox can come out WIDER than its max-content
        // (more lines at a narrower probe), and the content-sized wrapper
        // (`render::content_sized`) is a grid too (`col-wrap-012`).
        let max_content = tree.measure_child_size(
            node,
            Size {
                width: None,
                height,
            },
            containing_block_size.map(Option::Some),
            Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::Definite(available_border_box.height),
            },
            SizingMode::InherentSize,
            AbsoluteAxis::Horizontal,
            Line::FALSE,
        );
        // ★ MEASURED (03.10): `css-flexbox/intrinsic-size/row-004` went
        // 0.00 -> 2.08 here until the flex row min-content clamped a
        // non-growable item by its flex base size (§9.9.1, flexbox.rs,
        // 7c3bf67) — the floor itself was right.
        let floor = min_content.min(max_content);
        Some(max_content.min(floor.max(grid_area_minus_item_margins_size.width)))
    } else {
        width
    };

    // Clamp size by min and max width/height
    let Size { width, height } = Size { width, height }.maybe_clamp(min_size, max_size);

    // Layout node
    let size = if position.is_out_of_flow() && (width.is_none() || height.is_none()) {
        tree.measure_child_size_both(
            node,
            Size { width, height },
            containing_block_size.map(Option::Some),
            available_border_box.map(AvailableSpace::Definite),
            SizingMode::InherentSize,
            Line::FALSE,
        )
        .map(Some)
    } else {
        Size { width, height }
    };

    let mut layout_output = tree.perform_child_layout(
        node,
        size,
        containing_block_size.map(Option::Some),
        // Upstream #1234: a definite available space is the child's border-box space (the
        // parent subtracts the item's resolved, possibly trimmed, margins; auto = 0). Baseline
        // shims are not subtracted here (KaminIDE behaviour before the upgrade).
        available_border_box.map(AvailableSpace::Definite),
        SizingMode::InherentSize,
        Line::FALSE,
    );

    // Resolve final size
    let Size { width, height } = size
        .unwrap_or(layout_output.size)
        .maybe_clamp(min_size, max_size);

    // Baseline x groups are physical: writing-mode projection supplies x_end.
    // Ordinary positional keywords still use native inline Direction.
    let x_alignment = alignment_styles.horizontal;
    // An absolutely-positioned box takes no part in baseline alignment: its
    // baseline values align it to the containing block's start/end edge like
    // the positional keywords (Blink `absolute_utils.cc` GetAlignmentInsetBias:
    // kBaseline -> InlineStart, kLastBaseline -> InlineEnd).
    let abspos_baseline = |a: AlignItems| match a.keyword {
        AlignItemsKeyword::Baseline if position.is_out_of_flow() => AlignItems {
            keyword: AlignItemsKeyword::Start,
            ..a
        },
        AlignItemsKeyword::LastBaseline if position.is_out_of_flow() => AlignItems {
            keyword: AlignItemsKeyword::End,
            ..a
        },
        _ => a,
    };
    let x_alignment = abspos_baseline(x_alignment);
    let x_alignment =
        super::baseline_orientation::projected_x_alignment(x_alignment, x_end, direction);
    let (x, x_margin) = align_item_within_area(
        Line {
            start: grid_area.left,
            end: grid_area.right,
        },
        x_alignment,
        width,
        position,
        inset_horizontal,
        margin.horizontal_components(),
        shims.left,
        shims.right,
        direction,
    );
    let (y, y_margin) = align_item_within_area(
        Line {
            start: grid_area.top,
            end: grid_area.bottom,
        },
        abspos_baseline(alignment_styles.vertical),
        height,
        position,
        inset_vertical,
        margin.vertical_components(),
        shims.top,
        shims.bottom,
        Direction::Ltr,
    );

    let scrollbar_size = Size {
        width: if overflow.y == Overflow::Scroll {
            scrollbar_width
        } else {
            0.0
        },
        height: if overflow.x == Overflow::Scroll {
            scrollbar_width
        } else {
            0.0
        },
    };

    let resolved_margin = Rect {
        left: x_margin.start,
        right: x_margin.end,
        top: y_margin.start,
        bottom: y_margin.end,
    };

    // Bubble out-of-flow candidates from the item's subtree, translating anchors from
    // item-relative to container-relative coordinates
    let mut item_candidates = layout_output.oof_candidates.take();
    if !item_candidates.is_empty() {
        item_candidates.translate(Point { x, y });
        bubbled_candidates.append(&mut item_candidates);
    }

    tree.set_unrounded_layout(
        node,
        &Layout {
            order,
            location: Point { x, y },
            size: Size { width, height },
            #[cfg(feature = "content_size")]
            scrollable_overflow_rect: layout_output.scrollable_overflow_rect,
            scrollbar_size,
            padding,
            border,
            margin: resolved_margin,
        },
    );

    #[cfg(feature = "content_size")]
    let contribution = {
        // Contributions to the container's scrollable overflow rect are measured from the
        // container's padding-box origin (mirrored for RTL), matching the scrollable overflow region.
        let contribution_location = if direction.is_rtl() {
            Point {
                x: container_border_box_width - (x + width) - container_border.right,
                y: y - container_border.top,
            }
        } else {
            Point {
                x: x - container_border.left,
                y: y - container_border.top,
            }
        };
        compute_scrollable_overflow_contribution(
            contribution_location,
            Size { width, height },
            layout_output.scrollable_overflow_rect,
            overflow,
            contain,
            container_is_scroll_container,
        )
    };
    #[cfg(not(feature = "content_size"))]
    let contribution = Rect::ZERO;

    (
        contribution,
        y,
        height,
        layout_output.baselines.first,
        layout_output.last_or_first_y(),
    )
}

/// Align and size a grid item along a single axis
#[allow(clippy::too_many_arguments)]
pub(super) fn align_item_within_area(
    grid_area: Line<f32>,
    alignment_style: AlignSelf,
    resolved_size: f32,
    position: Position,
    inset: Line<Option<f32>>,
    margin: Line<Option<f32>>,
    baseline_shim: f32,
    baseline_shim_end: f32,
    direction: Direction,
) -> (f32, Line<f32>) {
    // Calculate grid area dimension in the axis
    let non_auto_margin = Line {
        start: margin.start.unwrap_or(0.0) + baseline_shim,
        end: margin.end.unwrap_or(0.0) + baseline_shim_end,
    };
    let grid_area_size = f32_max(grid_area.end - grid_area.start, 0.0);
    let free_space = f32_max(grid_area_size - resolved_size - non_auto_margin.sum(), 0.0);

    // Expand auto margins to fill available space
    let auto_margin_count = margin.start.is_none() as u8 + margin.end.is_none() as u8;
    // Static-position abspos auto margins do not displace the alignment subject.
    let static_abs = position.is_out_of_flow() && inset.start.is_none() && inset.end.is_none();
    let auto_margin_size = if auto_margin_count > 0 && !static_abs {
        free_space / auto_margin_count as f32
    } else {
        0.0
    };
    let resolved_margin = Line {
        start: margin.start.unwrap_or(auto_margin_size) + baseline_shim,
        end: margin.end.unwrap_or(auto_margin_size) + baseline_shim_end,
    };

    let overflows = resolved_size + non_auto_margin.sum() > grid_area_size;
    let alignment_keyword = resolve_self_alignment_safety(alignment_style, overflows);

    // Compute offset in the axis
    let alignment_based_offset = match alignment_keyword {
        // TODO: Add support for baseline alignment. For now we treat it as "start".
        AlignItemsKeyword::Start
        | AlignItemsKeyword::FlexStart
        | AlignItemsKeyword::Baseline
        | AlignItemsKeyword::Stretch => {
            if direction.is_rtl() {
                grid_area_size - resolved_size - resolved_margin.end
            } else {
                resolved_margin.start
            }
        }
        // Local last-baseline shims name physical end edges after writing-mode projection.
        AlignItemsKeyword::LastBaseline => grid_area_size - resolved_size - resolved_margin.end,
        AlignItemsKeyword::End | AlignItemsKeyword::FlexEnd => {
            if direction.is_rtl() {
                resolved_margin.start
            } else {
                grid_area_size - resolved_size - resolved_margin.end
            }
        }
        AlignItemsKeyword::Center => {
            (grid_area_size - resolved_size + resolved_margin.start - resolved_margin.end) / 2.0
        }
        // Normal is resolved, and SelfStart/SelfEnd are resolved to Start/End against
        // the item's own direction, in `align_and_position_item`.
        AlignItemsKeyword::Normal | AlignItemsKeyword::SelfStart | AlignItemsKeyword::SelfEnd => {
            unreachable!()
        }
    };

    let offset_within_area = if position.is_out_of_flow() {
        match (inset.start, inset.end) {
            (Some(start), Some(end)) => {
                if direction.is_rtl() {
                    grid_area_size - end - resolved_size - non_auto_margin.end
                } else {
                    start + non_auto_margin.start
                }
            }
            (Some(start), None) => start + non_auto_margin.start,
            (None, Some(end)) => grid_area_size - end - resolved_size - non_auto_margin.end,
            (None, None) => alignment_based_offset,
        }
    } else {
        alignment_based_offset
    };

    let mut start = grid_area.start + offset_within_area;
    if position == Position::Relative {
        let relative_inset = if direction.is_rtl() {
            inset.end.map(|pos| -pos).or(inset.start)
        } else {
            inset.start.or(inset.end.map(|pos| -pos))
        };
        start += relative_inset.unwrap_or(0.0);
    }

    (start, resolved_margin)
}

#[cfg(test)]
mod kamin_tests {
    use super::*;

    fn place(
        align: AlignSelf,
        size: f32,
        margin: Line<Option<f32>>,
        shims: Line<f32>,
        direction: Direction,
        absolute: bool,
    ) -> (f32, Line<f32>) {
        align_item_within_area(
            Line {
                start: 10.0,
                end: 110.0,
            },
            align,
            size,
            if absolute {
                Position::Absolute
            } else {
                Position::Relative
            },
            Line {
                start: None,
                end: None,
            },
            margin,
            shims.start,
            shims.end,
            direction,
        )
    }

    #[test]
    fn first_and_last_shims_are_kept_on_separate_edges() {
        let margins = Line {
            start: Some(2.0),
            end: Some(3.0),
        };
        let shims = Line {
            start: 5.0,
            end: 7.0,
        };
        assert_eq!(
            place(
                AlignSelf::BASELINE,
                20.0,
                margins,
                shims,
                Direction::Ltr,
                false
            )
            .0,
            17.0
        );
        assert_eq!(
            place(
                AlignSelf::LAST_BASELINE,
                20.0,
                margins,
                shims,
                Direction::Ltr,
                false
            )
            .0,
            80.0
        );
    }

    #[test]
    fn rtl_safe_end_overflow_falls_back_to_inline_start() {
        let margins = Line {
            start: Some(0.0),
            end: Some(0.0),
        };
        assert_eq!(
            place(
                AlignSelf::SAFE_END,
                120.0,
                margins,
                Line {
                    start: 0.0,
                    end: 0.0
                },
                Direction::Rtl,
                false
            )
            .0,
            -10.0
        );
    }

    #[test]
    fn static_abspos_auto_margins_do_not_force_centering() {
        let (_, margins) = place(
            AlignSelf::START,
            20.0,
            Line {
                start: None,
                end: None,
            },
            Line {
                start: 0.0,
                end: 0.0,
            },
            Direction::Ltr,
            true,
        );
        assert_eq!(
            margins,
            Line {
                start: 0.0,
                end: 0.0
            }
        );
    }

    #[test]
    fn ordinary_inflow_auto_margins_still_absorb_free_space() {
        let (_, margins) = place(
            AlignSelf::START,
            20.0,
            Line {
                start: None,
                end: None,
            },
            Line {
                start: 0.0,
                end: 0.0,
            },
            Direction::Ltr,
            false,
        );
        assert_eq!(
            margins,
            Line {
                start: 40.0,
                end: 40.0
            }
        );
    }
}
