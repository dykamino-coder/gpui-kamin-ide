//! Отрисовать файл разметки и сохранить снимок — вторая половина стенда
//! попиксельного сравнения с браузером.
//!
//!     cargo run -p kamin-html --example compare -- <файл.html> <ширина> <высота>
//!
//! Окно открывается ровно заданного размера, без рамок и отступов, с белым
//! фоном — чтобы снимок отличался от браузерного только тем, что нарисовано,
//! а не тем, где оно нарисовано.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "process entry point owns console diagnostics"
)]

use gpui::{
    AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowDecorations,
    WindowOptions, div, point, px, rgb, size,
};
use kamin_html::{BROWSER_CSS, Document, RenderOpts};
use std::rc::Rc;

struct Page {
    doc: Result<Rc<Document>, kamin_html::DocumentFailure>,
}

impl Render for Page {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let doc = match &self.doc {
            Ok(doc) => doc,
            Err(failure) => {
                let status = kamin_html::diagnostics::DisplayStatus::Failed(*failure);
                return div().children(status.message()).into_any_element();
            }
        };
        let mut text = window.text_style();
        text.color = rgb(0x1a1c23).into();
        // Тот же базовый шрифт, что у браузера по умолчанию: иначе замер
        // против Chrome мерит разницу шрифтов, а не раскладки.
        text.font_family = "Times New Roman".into();
        text.font_size = px(16.).into();
        let opts = RenderOpts {
            viewport: (
                f32::from(window.viewport_size().width),
                f32::from(window.viewport_size().height),
            ),
            text,
            // `line-height: normal` как в браузере, а не золотое сечение GPUI.
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_size(px(16.))
            .children(doc.status_message().map(|message| div().child(message)))
            .children(doc.render(&opts))
            .into_any_element()
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

    gpui_platform::application()
        .with_instrumentation(kamin_html::host::instrumentation::from_env())
        .run(move |cx| {
            // Серые (не ClearType) глифы: как до gpui-pre 0.3.8, точный RGB стенда.
            cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
            // Тот же служебный шрифт, что и в прогоне reftest-ов: без него замер
            // против Chrome врал бы на шрифте, а не на раскладке.
            if let Ok(bytes) = std::fs::read("vendor/wpt-parsing/fonts/Ahem.ttf") {
                let _ = cx.text_system().add_fonts(vec![bytes.into()]);
            }
            // Метрики `ch`/`ex` меряются по РЕАЛЬНОМУ шрифту — щуп ставится после
            // регистрации, иначе замер шёл бы по шрифту-подмене.
            let system = cx.text_system().clone();
            let fonts =
                kamin_html::metrics::FontContext::new(system.clone()).with_loader(move |bytes| {
                    let before: std::collections::HashSet<String> =
                        system.all_font_names().into_iter().collect();
                    system.add_fonts(vec![bytes.into()]).ok()?;
                    system
                        .all_font_names()
                        .into_iter()
                        .find(|name| !before.contains(name))
                });
            let doc = Document::builder(&html, BROWSER_CSS)
                .fonts(&fonts)
                .build()
                .map(Rc::new);
            // Консоль — представление typed диагностик, не их источник.
            let mut lines = String::new();
            let batch = match &doc {
                Ok(doc) => doc.take_diagnostics(),
                Err(failure) => kamin_html::diagnostics::DiagnosticBatch {
                    terminal: Some(*failure),
                    ..Default::default()
                },
            };
            kamin_html::diagnostics::console::write_batch(&mut lines, "", &batch);
            eprint!("{lines}");
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
