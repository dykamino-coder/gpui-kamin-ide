//! Fresh-tree reference paint at a retained scroll offset.

use super::*;

/// What `read` reads from the frame of a new tree built at `offset`.
pub(super) fn fresh_with<T>(
    cx: &mut TestAppContext,
    html: &str,
    offset: gpui::Point<Pixels>,
    read: impl FnOnce(&mut VisualTestContext) -> T,
) -> T {
    let mut app = cx.clone();
    let (view, fresh) = open(&mut app, html);
    root(&view, fresh).set_offset(offset);
    fresh.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document
                .update(&format!("{html}<!-- fresh -->"), "")
                .unwrap();
            cx.notify();
        })
    });
    frame(fresh);
    frame(fresh);
    assert_eq!(root(&view, fresh).offset(), offset);
    read(fresh)
}
