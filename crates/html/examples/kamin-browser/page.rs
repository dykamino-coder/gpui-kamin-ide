//! File loading preserves the engine resource base and replaces failed pages safely.

use crate::app::Browser;
use gpui::{Context, Focusable, Window, px, size};
use kamin_html::{BROWSER_CSS, Document, host::document_view::DocumentView};

impl Browser {
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selector_open = false;
        self.page = None;
        self.watch_current_file(window, cx);
        let Some(path) = &self.current else {
            cx.notify();
            return;
        };
        self.stamp = crate::paths::stamp(path);
        #[cfg(feature = "why-profiler")]
        if let Some(journal) = &self.journal {
            journal.open_document(path);
        }
        let read = || {
            std::fs::read_to_string(path)
                .map_err(|error| format!("Cannot read {}: {error}", path.display()))
        };
        #[cfg(feature = "why-profiler")]
        let source = if let Some(journal) = &self.journal {
            journal.phase("io", read)
        } else {
            read()
        };
        #[cfg(not(feature = "why-profiler"))]
        let source = read();
        let build = || {
            source.and_then(|html| {
                DocumentView::open(
                    Document::builder(&html, BROWSER_CSS)
                        .fonts(&self.fonts)
                        .path(path),
                    size(px(self.viewport.0), px(self.viewport.1)),
                    window,
                    cx,
                )
                .map_err(|error| format!("Cannot build {}: {error}", path.display()))
            })
        };
        #[cfg(feature = "why-profiler")]
        let built = if let Some(journal) = &self.journal {
            journal.phase("document", build)
        } else {
            build()
        };
        #[cfg(not(feature = "why-profiler"))]
        let built = build();
        match built {
            Ok(page) => {
                #[cfg(feature = "why-profiler")]
                if let Some(journal) = &self.journal {
                    page.update(cx, |page, cx| page.set_journal(journal.clone(), window, cx));
                }
                if !self.list_focus.is_focused(window) {
                    page.read(cx).focus_handle(cx).focus(window, cx);
                }
                self.page = Some(page);
                self.error = None;
                window.set_window_title(&format!("Kamin Browser - {}", path.display()));
                eprintln!(
                    "Opened page: {} | viewport {} x {}",
                    path.display(),
                    self.viewport.0,
                    self.viewport.1
                );
            }
            Err(error) => {
                eprintln!("{error}");
                self.error = Some(error);
            }
        }
        cx.notify();
    }
}
