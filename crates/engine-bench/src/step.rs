//! Шаг документа перед кадром окна (`Document::update_rendering`): сборка и
//! раскладка идут до `Window::draw`, а их время ложится в те же фазы, что у
//! сборки в кадре (build, layout_request, layout).

use super::view::View;
use gpui::{Entity, Subscription, Window};
use kamin_html::{UpdatePhase, work::Clock};
use std::time::Instant;

/// Зарегистрировать шаг `view` в его окне.
pub(super) fn register(view: &Entity<View>, window: &mut Window) -> Subscription {
    // Документ — корень рендера view (`View::render`, `Timed` без id).
    let parent = gpui::ElementPlace::of_view(view.entity_id());
    let view = view.downgrade();
    window.on_update_rendering(move |window, cx| {
        let _ = view.update(cx, |view, cx| {
            // Опции кадра строятся в шаге, как прежде в render: в фазе сборки.
            kamin_html::work::phase!(_work, BuildElements);
            let opts = view.opts(window);
            let phases = view.phases.clone();
            {
                let mut phases = phases.borrow_mut();
                let started = Instant::now();
                phases.request_to_render = phases
                    .requested
                    .map(|request| started.duration_since(request));
            }
            let mut mark = Clock::now();
            view.doc
                .update_rendering(&opts, &parent, window, cx, |phase| {
                    let spent = mark.elapsed();
                    mark = Clock::now();
                    let mut phases = phases.borrow_mut();
                    match phase {
                        UpdatePhase::Built => {
                            phases.build += spent;
                            kamin_html::work::switch!(LayoutRequest);
                        }
                        UpdatePhase::Requested => {
                            phases.request_layout += spent;
                            kamin_html::work::switch!(Layout);
                        }
                        UpdatePhase::LaidOut => {
                            // Часть раскладки кадра; остаток добавит `Timed`.
                            phases.layout = Some(phases.layout.unwrap_or_default() + spent);
                            kamin_html::work::switch!(BuildElements);
                        }
                    }
                });
            view.step_opts = Some(opts);
        });
    })
}
