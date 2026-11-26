//! Opt-in real-window timing driver. Normal viewer startup does not enable tracing.

#[path = "timing_counters.rs"]
mod counters;
#[path = "timing_cpu.rs"]
mod cpu;
#[path = "timing_frames.rs"]
mod frames;
#[path = "timing_sample.rs"]
mod sample;
use crate::{app::Browser, cli::TimeOptions, folder, paths};
pub use cpu::Cpu;
use gpui::{App, Entity, Window};
pub use sample::Timing;
use std::time::{Duration, Instant};

pub fn start(view: &Entity<Browser>, options: TimeOptions, window: &mut Window, cx: &mut App) {
    gpui::profiler::set_trace_enabled(true);
    let listed = Instant::now();
    let listed_folder = paths::absolute(&options.folder)
        .and_then(|path| paths::list(&path).map(|files| (path, files)));
    let (root, files) = match listed_folder {
        Ok(listed) => listed,
        Err(error) => {
            eprintln!("TIMING_ERROR {error}");
            cx.quit();
            return;
        }
    };
    eprintln!(
        "TIMING_CONFIG {}",
        serde_json::json!({"width": options.width,
        "height": options.height, "base_only": options.base_only, "plain": options.plain,
        "list_ms": sample::ms(listed.elapsed()), "profile_debug_assertions": cfg!(debug_assertions),
        "idle": "first GPUI idle after final presentation, confirmed by 250ms without presentations",
        "phases": "GPUI window counters include chrome; taffy/measure/shape overlap draw and document_step; raster_submit includes swap; no GPU readback",
        "request_scope": "per-page open excludes process startup, font-context setup, and folder discovery",
        "cpu_scope": "all process threads through settled report, including the idle confirmation; Windows GetProcessTimes granularity applies; null when unavailable"})
    );
    let queue = folder::filtered(&files, options.base_only);
    if queue.is_empty() {
        eprintln!("TIMING_ERROR No matching HTML pages in {}", root.display());
        cx.quit();
        return;
    }
    view.update(cx, |view, cx| {
        view.folder = Some(root);
        view.base_only = options.base_only;
        view.all_files = files;
        view.files = queue.clone();
        view.timing_plain = options.plain;
        if !options.plain {
            view.list_focus.focus(window, cx);
        }
        cx.notify();
    });
    let view = view.downgrade();
    window
        .spawn(cx, async move |cx| {
            // Let initial chrome bounds settle before the first open request.
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            for (index, path) in queue.into_iter().enumerate() {
                let opened = view.update_in(cx, |view, window, cx| {
                    view.viewport = (options.width as f32, options.height as f32);
                    view.current = Some(path.clone());
                    view.selected = index;
                    view.timing = Some(Timing::new(view.viewport, window, cx));
                    view.reload(window, cx);
                });
                if opened.is_err() {
                    return;
                }
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(25))
                        .await;
                    let finished = view.update_in(cx, |view, _, cx| {
                        let Some(timing) = &mut view.timing else {
                            return true;
                        };
                        let settled = timing.poll(cx);
                        let failed = view.error.is_some();
                        if !settled
                            && !failed
                            && timing.started.elapsed() < Duration::from_secs(120)
                        {
                            return false;
                        }
                        let document = view.page.as_ref().map(|page| page.read(cx).document());
                        eprintln!(
                            "TIMING_PAGE {}",
                            timing.report(
                                &path,
                                view.error.as_deref(),
                                document.map(|doc| doc.retained_frames()),
                                document.map(|doc| doc.builds_inside_draw()),
                                settled
                            )
                        );
                        true
                    });
                    match finished {
                        Ok(false) => {}
                        Ok(true) => break,
                        Err(_) => return,
                    }
                }
            }
            let _ = cx.update(|_, cx| cx.quit());
        })
        .detach();
}
