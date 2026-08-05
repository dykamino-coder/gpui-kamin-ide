//! Отрисовать файл разметки и сохранить снимок — вторая половина стенда
//! попиксельного сравнения с браузером.
//!
//!     cargo run -p kamin-html --example compare -- <файл.html> <ширина> <высота>
//!
//! Окно открывается ровно заданного размера, без рамок и отступов, с белым
//! фоном — чтобы снимок отличался от браузерного только тем, что нарисовано,
//! а не тем, где оно нарисовано.

use gpui::{
    AppContext as _, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    Styled, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowOptions, div, point, px, rgb, size,
};
use kamin_html::{Document, RenderOpts, render};
use std::rc::Rc;

struct Page {
    doc: Rc<Document>,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut text = window.text_style();
        text.color = rgb(0x1a1c23).into();
        let opts = RenderOpts {
            text,
            table_min_col: 80.,
        };
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .children(render(self.doc.nodes(), &opts))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned().unwrap_or_default();
    let w: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800.);
    let h: f32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(700.);
    let html = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("не прочитан {path}: {e}");
        std::process::exit(1);
    });

    Application::new().run(move |cx| {
        let doc = Rc::new(Document::new(&html, ""));
        cx.open_window(
            WindowOptions {
                // Точка в углу экрана и точный размер: снимок сравнивается с
                // браузерным по координатам, поэтому окно не должно
                // центрироваться или подгоняться.
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(40.), px(40.)),
                    size: size(px(w), px(h)),
                })),
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_decorations: Some(WindowDecorations::Client),
                window_background: WindowBackgroundAppearance::Opaque,
                ..Default::default()
            },
            |_, cx| -> Entity<Page> { cx.new(|_| Page { doc }) },
        )
        .unwrap();
        cx.activate(true);
    });
}
