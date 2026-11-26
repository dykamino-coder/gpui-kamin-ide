//! Standalone local-page viewer: native file/folder opening and engine-owned scrolling.
//!
//! Run: cargo run -p kamin-html --example kamin-browser -- [page.html | folder]

#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(
    clippy::print_stderr,
    reason = "viewer reports file loading and startup errors"
)]

#[path = "kamin-browser/app.rs"]
mod app;
#[path = "kamin-browser/dialog.rs"]
mod dialog;
#[path = "kamin-browser/page.rs"]
mod page;
#[path = "kamin-browser/paths.rs"]
mod paths;
#[path = "kamin-browser/selector.rs"]
mod selector;
#[path = "kamin-browser/toolbar.rs"]
mod toolbar;
#[path = "kamin-browser/ui.rs"]
mod ui;
#[path = "kamin-browser/viewport.rs"]
mod viewport;
#[path = "kamin-browser/window.rs"]
mod window;

use gpui::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use std::{collections::HashSet, path::PathBuf};

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    gpui_platform::application()
        .with_instrumentation(kamin_html::host::instrumentation::from_env())
        .run(move |cx| {
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
            let bounds = Bounds::centered(None, size(px(1280.), px(900.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Kamin Browser".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::Browser::new(path, fonts, window, cx)),
            );
            if let Err(error) = opened {
                eprintln!("Cannot open viewer: {error}");
                cx.quit();
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}
