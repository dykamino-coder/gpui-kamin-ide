//! A standalone browser seed: open a local HTML page, scroll its root, resize its viewport.
//!
//! Run: cargo run -p kamin-html --example kamin-browser -- page.html

#![allow(
    clippy::print_stderr,
    reason = "process entry point reports startup failures"
)]

use gpui::{
    AppContext, Bounds, Context, Entity, Focusable, IntoElement, ParentElement, Render,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};
use kamin_html::{BROWSER_CSS, Document, host::document_view::DocumentView};
use std::collections::HashSet;
#[path = "kamin-browser/args.rs"]
mod args;

struct Browser {
    page: Result<Entity<DocumentView>, kamin_html::DocumentFailure>,
}

impl Render for Browser {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.page {
            Ok(page) => page.clone().into_any_element(),
            Err(error) => div()
                .child(format!("Cannot build page: {error}"))
                .into_any_element(),
        }
    }
}

fn main() {
    let args = match args::Args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!(
                "{error}\nUsage: kamin-browser <page.html> [--viewport WIDTHxHEIGHT] [--why DIR]"
            );
            return;
        }
    };
    let progress = match kamin_html::host::journal::start(args.why.as_deref()) {
        Ok(progress) => progress,
        Err(error) => {
            eprintln!("Cannot start journal: {error}");
            return;
        }
    };
    let path = args.path;
    let reading = kamin_html::why::Span::new("host.read_document");
    let html = match std::fs::read_to_string(&path) {
        Ok(html) => html,
        Err(error) => {
            eprintln!("Cannot read {}: {error}", path.display());
            return;
        }
    };
    drop(reading);
    gpui_platform::application()
        .with_instrumentation(kamin_html::host::instrumentation::from_env())
        .run(move |cx| {
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
            let system = cx.text_system().clone();
            let fonts =
                kamin_html::metrics::FontContext::new(system.clone()).with_loader(move |bytes| {
                    let before: HashSet<_> = system.all_font_names().into_iter().collect();
                    system.add_fonts(vec![bytes.into()]).ok()?;
                    system
                        .all_font_names()
                        .into_iter()
                        .find(|name| !before.contains(name))
                });
            let bounds = Bounds::centered(
                None,
                size(px(args.viewport.0 as f32), px(args.viewport.1 as f32)),
                cx,
            );
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(format!("Kamin Browser - {}", path.display()).into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    cx.new(|cx| {
                        let view = DocumentView::open(
                            Document::builder(&html, BROWSER_CSS)
                                .fonts(&fonts)
                                .path(&path),
                            window.viewport_size(),
                            window,
                            cx,
                        )
                        .inspect_err(|error| eprintln!("Cannot build page: {error}"));
                        if let Ok(view) = &view {
                            view.update(cx, |view, cx| {
                                cx.observe_window_bounds(window, |view, window, cx| {
                                    if let Err(error) = view.resize(window.viewport_size(), cx) {
                                        eprintln!("Cannot resize page: {error}");
                                    }
                                })
                                .detach();
                                view.focus_handle(cx).focus(window, cx);
                            });
                        }
                        Browser { page: view }
                    })
                },
            );
            if let Err(error) = opened {
                eprintln!("Cannot open page: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    if let Some(progress) = progress
        && let Err(error) = progress.finish()
    {
        eprintln!("Cannot finish journal: {error}");
    }
}
