//! Demo frame rendering, document scroll area and block-count footer.

use super::{Demo, INK};
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use kamin_html::{RenderOpts, render_block, scroll::scroll_area};

impl Render for Demo {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Стиль текста снимается с цветом корня — иначе прогоны красятся
        // цветом по умолчанию (тёмным на тёмном фоне).
        let mut text = window.text_style();
        text.color = rgb(INK).into();
        let opts = RenderOpts {
            viewport: (
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            ),
            text: text.clone(),
            // `line-height: normal` как в браузере, а не золотое сечение GPUI.
            normal_line_height: 1.31,
            doc_salt: 0,
        };
        div()
            .size_full()
            .bg(rgb(0x1e1f29))
            .text_color(rgb(INK))
            .text_size(px(13.))
            .p(px(18.))
            .flex()
            .flex_col()
            .child(scroll_area(
                "doc",
                &self.scroll,
                (0..self.doc.top_level_blocks()).filter_map(|ix| {
                    // Блоки собираются по одному — той же функцией, которой их
                    // берёт виртуализованный список в реальном приложении.
                    render_block(self.doc.nodes(), ix, &opts)
                }),
            ))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(11.))
                    .text_color(rgb(0x9aa0b4))
                    .child(format!(
                        "блоков верхнего уровня: {} · узлов в документе: {}",
                        self.doc.top_level_blocks(),
                        self.doc.node_count()
                    )),
            )
    }
}
