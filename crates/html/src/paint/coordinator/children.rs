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
    ChildPainter(Rc::new(move |context, children, window, cx, stack| {
        Box::pin(async move {
            match crate::paint::raster::background(&context, window) {
                Some(background) => {
                    window
                        .with_raster_context_async(Some(background), async |window| {
                            paint(policy, context.style, children, window, cx, stack).await;
                        })
                        .await
                }
                None => paint(policy, context.style, children, window, cx, stack).await,
            }
        })
    }))
}

async fn paint(
    policy: Policy,
    style: &Style,
    children: &mut dyn Iterator<Item = &mut AnyElement>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    super::segments::remember_clips(window);
    let pass = current(window);
    let open = pass.as_ref().is_some_and(|pass| pass.is_open(window));
    if open {
        let pass = pass.expect("open pass");
        let stacking = policy.stacking || style.opacity.is_some_and(|opacity| opacity < 1.0);
        let guard = (stacking || policy.atomic).then(|| pass.open(window));
        for child in children {
            child.paint_async(window, cx, stack).await;
        }
        if let Some(guard) = guard {
            if stacking {
                pass.drain_async(guard.depth, window, cx, stack).await;
            } else {
                pass.finish_atomic_async(guard.depth, window, cx, stack)
                    .await;
            }
        }
    } else {
        fallback(children, window, cx, stack).await;
    }
}

// Список fallback не занимает стек каждого открытого CSS collector.
#[inline(never)]
async fn fallback(
    children: &mut dyn Iterator<Item = &mut AnyElement>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    let mut late = Vec::new();
    for child in children {
        if child.paint_metadata().is_some_and(deferred::positioned) {
            late.push(child);
        } else {
            child.paint_async(window, cx, stack).await;
        }
    }
    for child in late {
        child.paint_async(window, cx, stack).await;
    }
}
