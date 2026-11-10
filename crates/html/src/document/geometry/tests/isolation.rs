//! Вложенный запрос не наследует идентичность и шрифты внешнего элемента.

use super::*;
use gpui::{TextStyle, TextStyleRefinement, px};

#[gpui::test]
fn geometry_query_isolates_identity_and_inherited_text(cx: &mut TestAppContext) {
    let (_, cx) = cx
        .add_window_view(|_, _| View(Rc::new(Document::builder("", BROWSER_CSS).build().unwrap())));
    cx.update(|window, cx| {
        window.with_layout_query(cx, |window, cx| {
            window.with_id("outer", |window| {
                window.with_text_style(
                    Some(TextStyleRefinement {
                        font_size: Some(px(71.).into()),
                        ..Default::default()
                    }),
                    |window| {
                        let outer = window.text_style().font_size;
                        window.with_layout_query(cx, |window, _| {
                            assert_eq!(
                                window.text_style().font_size,
                                TextStyle::default().font_size
                            );
                            window.with_global_id("inner".into(), |id, _| assert_eq!(id.len(), 1));
                        });
                        assert_eq!(window.text_style().font_size, outer);
                        window.with_global_id("inner".into(), |id, _| assert_eq!(id.len(), 2));
                    },
                );
            });
        });
    });
}
