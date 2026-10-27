//! CSS-правила обхода детей коробки внутри её GPUI эффектов и clips.

use super::*;
use gpui::{AnyElement, App, ChildPainter, Div, Style};

#[derive(Clone, Copy, Default)]
struct Policy {
    atomic: bool,
    stacking: bool,
}

pub(crate) fn atomic(div: Div) -> Div {
    box_policy(div, true, false)
}

pub(crate) fn box_policy(div: Div, atomic: bool, stacking: bool) -> Div {
    if atomic || stacking {
        div.paint_children_with(painter(Policy { atomic, stacking }))
    } else {
        div
    }
}

pub(super) fn default_painter() -> ChildPainter {
    painter(Policy::default())
}

fn painter(policy: Policy) -> ChildPainter {
    ChildPainter(Rc::new(
        move |context, children, window, cx| match crate::paint::raster::background(context, window)
        {
            Some(background) => window.with_raster_context(Some(background), |window| {
                paint(policy, context.style, children, window, cx);
            }),
            None => paint(policy, context.style, children, window, cx),
        },
    ))
}

fn paint(
    policy: Policy,
    style: &Style,
    children: &mut dyn Iterator<Item = &mut AnyElement>,
    window: &mut Window,
    cx: &mut App,
) {
    super::segments::remember_clips(window);
    let pass = current(window);
    let open = pass.as_ref().is_some_and(|pass| pass.is_open(window));
    if open {
        let pass = pass.expect("open pass");
        let stacking = policy.stacking || style.opacity.is_some_and(|opacity| opacity < 1.0);
        let guard = (stacking || policy.atomic).then(|| pass.open(window));
        for child in children {
            child.paint(window, cx);
        }
        if let Some(guard) = guard {
            if stacking {
                pass.drain(guard.depth, window, cx);
            } else {
                pass.finish_atomic(guard.depth, window, cx);
            }
        }
    } else {
        fallback(children, window, cx);
    }
}

// Список fallback не занимает стек каждого открытого CSS collector.
#[inline(never)]
fn fallback(
    children: &mut dyn Iterator<Item = &mut AnyElement>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut late = Vec::new();
    for child in children {
        if child
            .paint_metadata()
            .is_some_and(|tag| tag.is::<deferred::PositionedTag>())
        {
            late.push(child);
        } else {
            child.paint(window, cx);
        }
    }
    for child in late {
        child.paint(window, cx);
    }
}
