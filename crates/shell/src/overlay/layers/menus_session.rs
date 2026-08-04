//! Меню новой сессии и переполнения табов.
//!
//! Слой вынесен из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::overlay::{dropdown_shadow, hit_area};
use crate::root::RootView;
use gpui::prelude::*;

use gpui::Div;

pub(crate) fn add_menus_session(
    mut layer: Div,
    r: &RootView,
    p: &'static kamin_theme::Palette,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    _vh: f32,
    _window: &mut gpui::Window,
) -> Div {
    if let Some((nx, ny)) = r.new_session_menu {
        // «+»-меню (.picker 1:1): иконка 14 + gap 8, item 6/8, hover 10%
        let item = |id: &'static str,
                    glyph: &'static str,
                    label: &'static str,
                    ev: ShellEvent,
                    tx: smol::channel::Sender<ShellEvent>,
                    p: &kamin_theme::Palette| {
            let hb = {
                let mut c = crate::colors::rgba(p.text_primary);
                c.a = 0.10;
                c
            };
            gpui::div()
                .id(id)
                .flex()
                .items_center()
                .gap(gpui::px(kamin_metrics::SPACE_2))
                .px(gpui::px(kamin_metrics::SPACE_2))
                .py(gpui::px(6.0))
                .rounded(gpui::px(kamin_metrics::RADIUS_SM))
                .text_size(gpui::px(kamin_metrics::FS_SM))
                .whitespace_nowrap()
                .text_color(crate::colors::rgba(p.text_secondary))
                .cursor_pointer()
                .hover(move |s| s.bg(hb).text_color(crate::colors::rgba(p.text_primary)))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ev.clone());
                })
                .child(crate::ui::icon::codicon(glyph, 14.0))
                .child(label)
        };
        layer = layer.child(
            gpui::div()
                .id("new-session-menu")
                .occlude()
                .absolute()
                // Якорь — ПРЯМОУГОЛЬНИК кнопки «+», как `SessionTabs.tsx:46`
                // (`left = btn.left`, `top = btn.bottom + PICKER_GAP_PX 4`),
                // а не координаты курсора (ревью ц.13)
                .left(gpui::px(
                    crate::probe::registry::bounds_of("new-session")
                        .map(|[bx, _, _, _]| bx)
                        .unwrap_or(nx - 8.0)
                        .clamp(8.0, vw - 218.0),
                ))
                .top(gpui::px(
                    crate::probe::registry::bounds_of("new-session")
                        .map(|[_, by, _, bh]| by + bh + 4.0)
                        .unwrap_or(ny + 16.0),
                ))
                .min_w(gpui::px(200.0))
                .flex()
                .flex_col()
                .p(gpui::px(kamin_metrics::SPACE_1))
                .rounded(gpui::px(kamin_metrics::RADIUS_MD))
                .bg(crate::colors::rgba(p.bg_surface))
                .border_1()
                .border_color({
                    let mut c = crate::colors::rgba(p.text_primary);
                    c.a = 0.06;
                    c
                })
                .shadow(dropdown_shadow())
                .child(hit_area())
                .child(item(
                    "ns-folder",
                    "\u{eaf7}", // codicon-folder-opened
                    "New session (folder…)",
                    ShellEvent::NewSessionInFolderPrompt,
                    tx.clone(),
                    p,
                ))
                .child(item(
                    "ns-empty",
                    "\u{ebb5}", // codicon-circle-large-outline
                    "No folder session",
                    ShellEvent::NewEmptySession,
                    tx.clone(),
                    p,
                )),
        );
    }

    if let Some((ox, oy)) = r.tabs_overflow_open {
        // Оверфлоу-дропдаун табов сессий (ВСЕ поповеры — в overlay)
        let ids = crate::ui::session_tabs::overflow_hidden_ids()
            .lock()
            .unwrap()
            .clone();
        let items: Vec<(String, String, Option<String>)> = r
            .sessions
            .as_ref()
            .map(|snap| {
                ids.iter()
                    .filter_map(|id| {
                        snap.sessions
                            .iter()
                            .find(|s| &s.id == id)
                            .map(|s| (s.id.clone(), s.name.clone(), s.color.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        layer = layer.child(crate::ui::session_tabs::tabs_overflow_menu(
            &items, ox, oy, vw, tx, p,
        ));
    }
    layer
}
