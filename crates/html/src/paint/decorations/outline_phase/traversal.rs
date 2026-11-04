//! Области отложенных контуров сохраняются до завершения paint-потомка.

use super::*;

pub(crate) async fn paint_async(
    owner: Option<&Rc<Deferred>>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
    f: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
) {
    let Some(owner) = owner else {
        return f(window, cx, stack).await;
    };
    owner.outlines.borrow_mut().push(Some(Vec::new()));
    f(window, cx, stack).await;
    flush_async(owner, window, cx, stack).await;
}

pub(crate) async fn flow_async(
    owner: Option<&Rc<Deferred>>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
    f: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
) {
    if owner.is_some_and(|owner| matches!(owner.outlines.borrow().last(), Some(Some(_)))) {
        f(window, cx, stack).await;
    } else {
        paint_async(owner, window, cx, stack, f).await;
    }
}

pub(crate) async fn scoped_async(
    owner: Option<&Rc<Deferred>>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
    f: impl AsyncFnOnce(&mut Window, &mut App, &mut gpui::PaintStack),
) {
    if owner.is_none_or(|owner| owner.outlines.borrow().is_empty()) {
        f(window, cx, stack).await;
    } else {
        paint_async(owner, window, cx, stack, f).await;
    }
}

async fn flush_async(
    owner: &Rc<Deferred>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    let outlines = owner
        .outlines
        .borrow_mut()
        .pop()
        .flatten()
        .unwrap_or_default();
    for (mut outline, clip) in outlines {
        window
            .with_content_mask_replaced_async(clip, async |window| {
                outline.paint_async(window, cx, stack).await;
            })
            .await;
    }
}
