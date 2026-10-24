//! Подмена документа и проверка первого кадра; граница кадра сохраняется в capture.

use super::capture_dump::Shot;
use super::page::Page;
use super::reftest_meta::reftest_pages;
use super::show::Stage;
use gpui::{AppContext as _, AsyncApp};
use kamin_html::{BROWSER_CSS, Document};
use std::rc::Rc;

impl Stage {
    pub(super) fn prepare(
        &self,
        cx: &mut AsyncApp,
        html: &str,
        source: Option<&str>,
        print: bool,
        first: bool,
    ) {
        let parse_started = std::time::Instant::now();
        let _ = cx.update_window(self.window, |view, window, cx| {
            if let Ok(page) = view.downcast::<Page>() {
                page.update(cx, |page, cx| {
                    let fonts = page.doc.font_context();
                    page.doc = Rc::new(match source {
                        Some(path) => Document::builder(html, BROWSER_CSS)
                            .fonts(&fonts)
                            .path(std::path::Path::new(path))
                            .print(print)
                            .build(),
                        None => Document::builder(html, BROWSER_CSS)
                            .fonts(&fonts)
                            .print(print)
                            .build(),
                    });
                    page.select = reftest_pages(html);
                    cx.notify();
                });
            }
            if first {
                gpui::frame_capture::arm_first();
            }
            window.refresh();
        });
        super::phase_timing::emit(source, "parse", parse_started.elapsed(), 1);
    }

    pub(super) fn checked(
        &self,
        first: Option<Shot>,
        settled: Option<Shot>,
        source: Option<&str>,
    ) -> Option<Shot> {
        let Some(first) = first else { return settled };
        let equal = settled
            .as_ref()
            .and_then(|last| super::pixel_compare::compare(&first, last))
            .is_some_and(|difference| difference.pixels == 0);
        if !equal && let Some(source) = source {
            self.frame_errors.borrow_mut().push(source.to_owned());
            let stem = format!("{}--first-frame", super::capture_name::stem(0, source));
            super::capture_dump::pair(&stem, &[Some(first.clone()), settled], None);
        }
        Some(first)
    }
}
