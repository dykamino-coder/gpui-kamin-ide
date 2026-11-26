//! File loading preserves the engine resource base and replaces failed pages safely.

use crate::app::Browser;
use gpui::{Context, Focusable, Window, px, size};
use kamin_html::{BROWSER_CSS, Document, host::document_view::DocumentView};

impl Browser {
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selector_open = false;
        self.page = None;
        let Some(path) = &self.current else {
            cx.notify();
            return;
        };
        self.stamp = crate::paths::stamp(path);
        #[cfg(feature = "viewer-timing")]
        let read_at = std::time::Instant::now();
        #[cfg(feature = "viewer-timing")]
        let read_cpu = crate::timing::Cpu::read();
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("Cannot read {}: {error}", path.display()));
        #[cfg(feature = "viewer-timing")]
        let build_at = {
            if let Some(timing) = &mut self.timing {
                timing.io_ms = read_at.elapsed().as_secs_f64() * 1000.;
                timing.io_cpu_ms = read_cpu.elapsed_ms();
            }
            std::time::Instant::now()
        };
        #[cfg(feature = "viewer-timing")]
        let build_cpu = crate::timing::Cpu::read();
        let built = source.and_then(|html| {
            DocumentView::open(
                Document::builder(&html, BROWSER_CSS)
                    .fonts(&self.fonts)
                    .path(path),
                size(px(self.viewport.0), px(self.viewport.1)),
                window,
                cx,
            )
            .map_err(|error| format!("Cannot build {}: {error}", path.display()))
        });
        #[cfg(feature = "viewer-timing")]
        if let Some(timing) = &mut self.timing {
            timing.document_ms = build_at.elapsed().as_secs_f64() * 1000.;
            timing.document_cpu_ms = build_cpu.elapsed_ms();
            if let Ok(page) = &built {
                let phases = page.read(cx).document().build_timings();
                timing.parse_ms = phases.parse.as_secs_f64() * 1000.;
                timing.style_ms = phases.style.as_secs_f64() * 1000.;
            }
            timing.built(window, built.as_ref().ok());
        }
        match built {
            Ok(page) => {
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
