//! Leaf text block: a block box whose children are all text, built in one pass.
//!
//! The general route (`animated` → `element_stack` → `dispatch_element` →
//! `generic_box_stack` → `generic_children` → `blocks` → `blocks_flow`) pays a
//! boxed future and several copies of the options per box for steps that are
//! identities on such a box. `try_build` proves those identities from the same
//! inputs the route reads and builds the same tree directly: one styled box
//! holding one paragraph. Blink keeps the same split: a block whose children
//! are all inline runs `InlineNode` layout without a block child iteration.

use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::options::RenderOpts;
use crate::paint::box_tree::styled::styled_div_with;
use crate::paint::effects::transform::transformed_node;
use crate::render::blocks::{context, prepare_children, text_only};
use crate::render::element::{dispatch_element, merged_style};
use crate::render::first_letter_scope::Scope;
use crate::render::paragraph::route::paint_inline_step7;
use crate::style::computed::{BoxStyle, Position};
use crate::text::line_trim::Trim;
use gpui::{AnyElement, IntoElement, ParentElement, Styled};

#[cfg(test)]
mod tests;

/// Outcome of the leaf attempt once the element's style has been merged.
pub(in crate::render::blocks::flow) enum Leaf {
    Built(AnyElement),
    /// The merged style rules the straight line out. Merging happens once per
    /// element (it advances the frame's anchor sequence), so the general route
    /// continues from this style (`generic`).
    Generic(Box<BoxStyle>),
}

/// Tags with their own arm in `element_inner` or `dispatch_element`.
fn special_tag(e: &Element) -> bool {
    matches!(
        e.tag.as_str(),
        "input"
            | "textarea"
            | "select"
            | "progress"
            | "meter"
            | "li"
            | "canvas"
            | "img"
            | "embed"
            | "iframe"
            | "object"
            | "video"
            | "audio"
            | "svg"
            | "hr"
            | "table"
            | "ul"
            | "ol"
            | "html"
            | "body"
            | "html-viewport"
            | "fieldset"
            | "legend"
            | "math"
    ) || crate::layout::replaced::media::empty(e)
}

/// Everything the general route decides before merging the style is an
/// identity for this element under these options.
fn candidate(e: &Element, inherited: &BoxStyle, opts: &RenderOpts) -> bool {
    let used = e.style.used();
    let cx = &opts.cx;
    (e.anim.is_none() || used.reset_context.animation.is_none())
        && e.children.iter().all(|n| matches!(n, Node::Text(_)))
        && !special_tag(e)
        && used.reset_box_layout.display.is_none()
        && e.marks.shape_flow.is_none()
        && !(used.reset_svg_math.mathml && used.reset_svg_math.math_layout.is_some())
        && used.reset_grid.grid_areas.is_none()
        && used.reset_fragmentation.column_count.is_none()
        && used.reset_fragmentation.column_width.is_none()
        && used.reset_fragmentation.column_height.is_none()
        && cx.position.geometry_record.is_none()
        && cx.position.continuation_slots.is_none()
        && opts.frag.watch.is_none()
        && opts.frag.outer_space.is_none()
        && cx.values.viewport == opts.viewport
        && cx.contracts.reflow_depth < crate::layout::depth::MAX_REFLOW_DEPTH
        && crate::text::clamp::clamp_context(cx).is_none()
        && cx.clamp.frame.is_none()
        && cx.deferred.gap_items.is_none()
        && cx.inline.trim_requests.is_empty()
        && crate::interactive::scroll_extent::projection(
            cx.scroll.frame.as_ref(),
            inherited.self_node,
            None,
            cx.scroll.sticky.as_ref(),
        )
        .is_none()
}

/// The merged style keeps the box a plain block container whose children form
/// exactly one paragraph: no own layer, clamp, trim, containment collector or
/// reuse scope, and no blank text node that would end the inline run.
fn straight(e: &Element, merged: &BoxStyle) -> bool {
    let nodes = &e.children[..];
    let (ordered, _) = prepare_children::container_flags(merged);
    !merged.wm().vertical()
        && merged.reset_box_layout.display.is_none()
        && !matches!(
            merged.reset_positioned.position,
            Some(Position::Absolute | Position::Fixed)
        )
        && !crate::layout::box_kind::clamp_host(e, merged)
        && !crate::text::inline::establishes_cb(merged)
        && !merged.reset_context.icb
        && !merged.reset_text_box.text_box_trim_start
        && !merged.reset_text_box.text_box_trim_end
        && merged.reset_context.contain_layout != Some(true)
        && merged.reset_context.contain_paint != Some(true)
        && merged.reset_effects.will_change & crate::style::computed::wc::STACK == 0
        && merged.reset_transforms.preserve_3d != Some(true)
        && text_only::applies(nodes, merged, ordered)
        && nodes
            .iter()
            .enumerate()
            .all(|(idx, n)| super::inline_node::inline_node(n, merged, false, &nodes[..idx]))
}

/// `None` — the element is not a leaf candidate and nothing was touched: the
/// caller takes the general route from its start.
pub(in crate::render::blocks::flow) fn try_build(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> Option<Leaf> {
    if !candidate(e, inherited, opts) {
        return None;
    }
    crate::work::add!(RebuiltNodes, 1);
    let merged = merged_style(e, inherited, opts);
    if !straight(e, &merged) {
        return Some(Leaf::Generic(Box::new(merged)));
    }
    let mut d = styled_div_with(e, &merged, &opts.cx).flex().flex_col();
    d.style().block_baselines = Some(true);
    if !e.children.is_empty() {
        // One copy of the options carries the child sizes, the entered depth
        // and the paragraph's trim owner.
        let sizes = BoxDimensions::for_element(e, &merged);
        let mut child = context::child_options(&merged, &sizes, opts, false);
        child.cx.contracts.reflow_depth += 1;
        child.cx.inline.trim_para = Some((
            merged.self_node,
            super::trim::para_flags(Trim::default(), true, true),
        ));
        let para = Scope::new(&e.children, &merged).paragraph(&e.children, &merged, &child);
        d = d.child(paint_inline_step7(para, &child));
    }
    Some(Leaf::Built(transformed_node(
        d.into_any_element(),
        &e.style.used(),
        inherited,
        e.node_id,
        opts.cx.frame(),
    )))
}

/// The general route from the point where the style is already merged.
pub(in crate::render::blocks::flow) async fn generic(
    stk: &mut reblessive::Stk,
    e: &Element,
    merged: Box<BoxStyle>,
    inherited: &BoxStyle,
    opts: &RenderOpts,
) -> AnyElement {
    let entered = opts.inner(|cx| cx.contracts.reflow_depth += 1);
    let built = stk
        .run(|stk| dispatch_element(stk, e, *merged, inherited, &entered, None, None))
        .await;
    transformed_node(
        built,
        &e.style.used(),
        inherited,
        e.node_id,
        opts.cx.frame(),
    )
}
