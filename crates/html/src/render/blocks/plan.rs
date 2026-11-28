//! The prepared child list of a block container (`prepare`): box-tree fixups, margin
//! collapsing, first-line and first-letter routes, float hosts and the paint order of
//! positioned children. It reads the width only through tracked reads, so a container
//! keeps it across frames (`layout::reuse::FlowReuse::child_plan`).

use super::*;

/// What `blocks_flow` builds the children of a container from.
pub(super) struct ChildPlan {
    pub(super) nodes: Vec<Node>,
    /// Indices in `nodes` before which the pending text run closes.
    pub(super) run_breaks: Vec<usize>,
    pub(super) letter_scope: first_letter_scope::Scope,
    pub(super) ordered_context: bool,
    pub(super) under_tf: bool,
    /// A child is sticky: the flow starts with its probe.
    pub(super) sticky: bool,
}

/// The scalars of the context `prepare` reads besides the container's style and nodes.
pub(super) fn inputs(sizes: &BoxDimensions, opts: &RenderOpts, cell_bfc: bool) -> [u32; 5] {
    let bits = |v: Option<f32>| v.map_or(u32::MAX, f32::to_bits);
    [
        bits(crate::style::values::value::px_strict(sizes.preferred[0])),
        bits(opts.cx.constraints.avail_h),
        u32::from(cell_bfc),
        opts.base_size().to_bits(),
        u32::from(opts.cx.vcx().clearance_possible),
    ]
}

pub(super) fn prepare(
    nodes: &[Node],
    inherited: &BoxStyle,
    sizes: &BoxDimensions,
    opts: &RenderOpts,
    cell_bfc: bool,
) -> ChildPlan {
    let vcx = opts.cx.vcx();
    // `content-visibility: hidden`: the content is skipped entirely (css-contain-2 §4); the box
    // stays, without children.
    let (collapsed, ordered_context, under_tf) =
        prepare_children(nodes, inherited, sizes, opts, cell_bfc);
    // Floats and baseline alignment do NOT apply to a flex or grid item, as CSS says. Without
    // this rule `float: right` on a row item took it out of its parent's layout.
    let collapsed = ordered_children(collapsed, ordered_context, inherited, opts);
    let flex_ctx = matches!(
        inherited.reset_box_layout.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let letter_scope = first_letter_scope::Scope::new(&collapsed, inherited);
    // An `initial-letter` drop cap becomes a float node BEFORE wrapping: `wrap_floats` then
    // handles it like an authored float. `::first-letter` does not apply in flex and grid.
    let collapsed = if ordered_context {
        collapsed
    } else {
        initial_letter_float(
            first_letter_descendants::route(
                first_line_descendants::route(collapsed, inherited),
                inherited,
            ),
            inherited,
            opts,
        )
    };
    // §8.3.1: the first child's margin adjoins the top margin of its containing block only
    // when no border or padding separates them and the block starts no formatting context.
    let cb_top_open = !own_context_style(inherited)
        && zero_len(inherited.reset_box_layout.padding.top)
        && zero_len(inherited.borders().top);
    // The measured band host (`band_flow.rs`) only in a BLOCK container: in flex and grid
    // `float` does not apply (css-flexbox-1 §3, css-grid-1 §6.1). The host works in vertical
    // writing too: the plan is in logical axes, made physical at build. Float
    // sides are line-relative (left = line-left), independent of `direction` (css-writing-modes-4 §6.3), so
    // RTL hosts use the same plan; paragraphs handle RTL within the bands.
    // `sideways-lr` uses the same logical bands; physical holders and shape
    // profiles start at the bottom line edge (css-writing-modes-4 §6.3).
    // Page stacks still need the measured host's break opportunities before
    // they can fragment it (CSS Break 3 section 4.1). Deferred clamp children
    // retain the counting scope, including the float's independent BFC.
    let measured_ok = !flex_ctx
        && !matches!(
            inherited.reset_box_layout.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && (!inherited.wm().block_rl() || inherited.wm().vertical());
    let collapsed = replaced_used_style::inline_nodes(collapsed, || opts.cx.constraints.avail_w());
    let collapsed = by_layer(
        wrap_floats(
            collapsed,
            inherited,
            cb_top_open,
            match inherited.inh.font.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            },
            measured_ok,
            own_context_style(inherited) || inherited.reset_flex.flex_item,
            cell_bfc,
            || opts.cx.constraints.cb_width(),
            opts.cx.constraints.avail_h,
            vcx,
        ),
        flex_ctx,
    );
    // We render a block as a flex column whose children shrink by default; normal flow has no
    // such thing: a child taller than its parent must overflow, not shrink. So in flow the
    // children do not shrink unless the markup asked for it.
    let flex_context = matches!(
        inherited.reset_box_layout.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let collapsed = flow_children(collapsed, ordered_context, inherited, opts, flex_context);
    // CSS 2.1 §9.9 step 8: a positioned box paints OVER the flow content, and our paint order
    // is the order of children. In normal flow the top layer does it; in a grid or flex
    // container it is off (`!ordered_context` below), so an absolute child went under its
    // siblings. Moving it to the end is enough: it takes no place in the flow (grid layout
    // does not place it), so the move changes only the paint.
    // A moved absolute child still BREAKS the text run: each run of text children is its own
    // anonymous item (css-flexbox-1 §4, css-grid-2 §6.1) and the absolute child is not in it.
    // After the move the pieces "Two " and "lines" became neighbours and merged into one
    // paragraph (`anonymous-flex-item-004`, `anonymous-grid-item-001`). `run_breaks`: indices
    // in the new list before which the pending paragraph closes.
    let mut run_breaks: Vec<usize> = vec![];
    let collapsed: Vec<Node> = if ordered_context {
        let in_flow = |n: &Node| match n {
            Node::Element(e) => {
                e.style.used().reset_positioned.position
                    != Some(crate::style::computed::Position::Absolute)
                    || e.style
                        .used()
                        .reset_positioned
                        .z_index
                        .is_some_and(|z| z < 0)
            }
            Node::Text(_) => true,
        };
        let mut flow: Vec<Node> = vec![];
        let mut over: Vec<Node> = vec![];
        for n in collapsed {
            if in_flow(&n) {
                flow.push(n);
            } else {
                if run_breaks.last() != Some(&flow.len()) {
                    run_breaks.push(flow.len());
                }
                over.push(n);
            }
        }
        flow.into_iter().chain(over).collect()
    } else {
        collapsed
    };

    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => {
            e.style.used().reset_positioned.position
                == Some(crate::style::computed::Position::Sticky)
        }
        _ => false,
    });
    ChildPlan {
        nodes: collapsed,
        run_breaks,
        letter_scope,
        ordered_context,
        under_tf,
        sticky,
    }
}
