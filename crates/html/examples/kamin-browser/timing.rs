//! Drives real input and unchanged redraws; all measurements belong to `why`.

use crate::{app::Browser, cli::TimeOptions, folder, paths, viewport};
use gpui::{
    App, Entity, Modifiers, PlatformInput, ScrollDelta, ScrollWheelEvent, Window, point, px,
};
use kamin_html::why::WindowJournal;
use std::time::{Duration, Instant};

pub fn start(view: &Entity<Browser>, options: TimeOptions, window: &mut Window, cx: &mut App) {
    let listed = paths::absolute(&options.folder)
        .and_then(|root| paths::list(&root).map(|files| (root, files)));
    let (root, files) = match listed {
        Ok(listed) => listed,
        Err(error) => {
            eprintln!("Cannot list scenario: {error}");
            cx.quit();
            return;
        }
    };
    let queue = folder::filtered(&files, options.base_only);
    if queue.is_empty() {
        eprintln!("No matching HTML pages in {}", root.display());
        cx.quit();
        return;
    }
    let journal = WindowJournal::new(window, cx);
    view.update(cx, |view, cx| {
        if view.hud.visible {
            view.toggle_hud(window, cx);
        }
        view.folder = Some(root);
        view.base_only = options.base_only;
        view.all_files = files;
        view.files = queue.clone();
        view.timing_plain = options.plain;
        view.preset_width = Some(options.width);
        view.journal = Some(journal.clone());
        view.resize(window, cx);
    });
    let view = view.downgrade();
    window
        .spawn(cx, async move |cx| {
            // Settle chrome before opening; this delay is not a measured frame.
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            'pages: for (index, path) in queue.into_iter().enumerate() {
                for action in 0..=20 {
                    let requested = view.update_in(cx, |view, window, cx| {
                        if action == 0 {
                            view.current = Some(path.clone());
                            view.selected = index;
                            view.reload(window, cx);
                            view.resize(window, cx);
                            journal.metadata(serde_json::json!({"plain": options.plain}));
                        } else if action <= 10 {
                            journal.step("wheel", action);
                            let before = view
                                .page
                                .as_ref()
                                .map(|page| page.read(cx).document().root_scroll_offset());
                            let x = if options.plain {
                                0.
                            } else {
                                viewport::SIDEBAR_WIDTH
                            };
                            let y = if options.plain {
                                0.
                            } else {
                                viewport::TOOLBAR_HEIGHT
                            };
                            window.dispatch_event(
                                PlatformInput::ScrollWheel(ScrollWheelEvent {
                                    position: point(px(x + 40.), px(y + 40.)),
                                    delta: ScrollDelta::Lines(point(0., -3.)),
                                    modifiers: Modifiers::default(),
                                    touch_phase: gpui::TouchPhase::Moved,
                                }),
                                cx,
                            );
                            let after = view
                                .page
                                .as_ref()
                                .map(|page| page.read(cx).document().root_scroll_offset());
                            journal.metadata(serde_json::json!({"wheel_changed": before != after}));
                            // Short pages still get an unchanged frame, explicitly marked.
                            if before == after {
                                window.refresh();
                            }
                        } else {
                            journal.step("repaint", action - 10);
                            window.refresh();
                        }
                        view.error.clone()
                    });
                    let error = match requested {
                        Ok(error) => error,
                        Err(error) => {
                            journal.error(format!("Scenario window closed: {error}"));
                            break 'pages;
                        }
                    };
                    if let Some(error) = error {
                        journal.error(error);
                        break 'pages;
                    }
                    // Retry observation only: this never dirties or presents the window.
                    let deadline = Instant::now() + Duration::from_secs(120);
                    while !journal.step_presented() {
                        if Instant::now() >= deadline {
                            journal.error(format!(
                                "No Presented event for {} action {action}",
                                path.display()
                            ));
                            break 'pages;
                        }
                        cx.background_executor()
                            .timer(Duration::from_millis(1))
                            .await;
                    }
                }
                let _ = view.update_in(cx, |view, window, cx| {
                    if let Some(page) = &view.page {
                        journal.sample_memory("series_end", page.read(cx).document(), window, cx);
                    }
                });
            }
            match journal.write(&options.why) {
                Ok(()) => eprintln!("Engine journal: {}", options.why.join("why.json").display()),
                Err(error) => eprintln!("Cannot write engine journal: {error}"),
            }
            if options.idle {
                if let Err(error) = crate::idle_control::serve(&journal, &options.why).await {
                    journal.error(format!("Idle observation control: {error}"));
                    let _ = journal.write(&options.why);
                }
            }
            let _ = cx.update(|_, cx| cx.quit());
        })
        .detach();
}
