//! Standalone local-page viewer: native file/folder opening and engine-owned scrolling.
//!
//! Run: cargo run -p kamin-html --example kamin-browser -- [page.html | folder]
//! Timing: build with `--features why-profiler`, then run
//! `kamin-browser --time folder --why journal-dir [--base-only] [--width 800 --height 600]`.

#![cfg_attr(windows, windows_subsystem = "windows")]
#![allow(
    clippy::print_stderr,
    reason = "viewer reports file loading and startup errors"
)]

#[path = "kamin-browser/app.rs"]
mod app;
#[path = "kamin-browser/cli.rs"]
mod cli;
#[path = "kamin-browser/dialog.rs"]
mod dialog;
#[path = "kamin-browser/file_watch.rs"]
mod file_watch;
#[path = "kamin-browser/folder.rs"]
mod folder;
#[path = "kamin-browser/hud.rs"]
mod hud;
#[cfg(feature = "why-profiler")]
#[path = "kamin-browser/idle_control.rs"]
mod idle_control;
#[path = "kamin-browser/page.rs"]
mod page;
#[path = "kamin-browser/paths.rs"]
mod paths;
#[path = "kamin-browser/selector.rs"]
mod selector;
#[cfg(feature = "why-profiler")]
#[path = "kamin-browser/timing.rs"]
mod timing;
#[path = "kamin-browser/toolbar.rs"]
mod toolbar;
#[path = "kamin-browser/ui.rs"]
mod ui;
#[path = "kamin-browser/viewport.rs"]
mod viewport;
#[path = "kamin-browser/window.rs"]
mod window;
#[cfg(test)]
#[path = "kamin-browser/window_tests.rs"]
mod window_tests;

use gpui::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use std::collections::HashSet;

fn main() {
    let options = cli::parse(std::env::args_os().skip(1));
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
            let dimensions =
                options
                    .as_ref()
                    .ok()
                    .and_then(|o| o.time.as_ref())
                    .map_or((1280., 900.), |o| {
                        (
                            o.width as f32 + if o.plain { 0. } else { viewport::SIDEBAR_WIDTH },
                            o.height as f32
                                + if o.plain {
                                    0.
                                } else {
                                    viewport::TOOLBAR_HEIGHT
                                },
                        )
                    });
            let bounds = Bounds::centered(None, size(px(dimensions.0), px(dimensions.1)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Kamin Browser".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let path = options.as_ref().ok().and_then(|o| o.path.clone());
                    let view = cx.new(|cx| app::Browser::new(path, fonts, window, cx));
                    match options {
                        Ok(options) => {
                            #[cfg(feature = "why-profiler")]
                            if let Some(options) = options.time {
                                timing::start(&view, options, window, cx);
                            }
                            #[cfg(not(feature = "why-profiler"))]
                            if options.time.is_some() {
                                view.update(cx, |view, _| {
                                    view.error = Some(
                                        "Timing mode requires the why-profiler Cargo feature."
                                            .into(),
                                    )
                                });
                            }
                        }
                        Err(error) => view.update(cx, |view, _| view.error = Some(error)),
                    }
                    view
                },
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
