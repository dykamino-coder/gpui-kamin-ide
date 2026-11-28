//! A window scale transition must invalidate font-dependent retained records.

use super::tests::{frame, open, open_without_step};
use gpui::TestAppContext;

#[gpui::test]
fn scale_changes_font_inputs_with_and_without_the_document_step(cx: &mut TestAppContext) {
    for stepped in [true, false] {
        let (view, cx) = if stepped {
            open(cx, "<p>first line<br>second line</p>")
        } else {
            open_without_step(cx, "<p>first line<br>second line</p>")
        };
        frame(cx);
        let before = cx.update(|_, app| view.read(app).doc.state.values.font_generation());
        cx.update(|window, _| window.set_scale_factor(1.25));
        frame(cx);
        let changed = cx.update(|_, app| {
            let doc = &view.read(app).doc;
            assert_eq!(doc.state.text.device_scale(), 1.25);
            doc.state.values.font_generation()
        });
        assert!(changed > before);
        frame(cx);
        assert_eq!(
            cx.update(|_, app| view.read(app).doc.state.values.font_generation()),
            changed
        );
        cx.update(|window, _| window.set_scale_factor(1.0));
        frame(cx);
        cx.update(|_, app| {
            let doc = &view.read(app).doc;
            assert_eq!(doc.state.text.device_scale(), 1.0);
            assert!(doc.state.values.font_generation() > changed);
        });
    }
}
