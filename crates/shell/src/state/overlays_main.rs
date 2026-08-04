//! Что главное окно рисует ПОД оверлеями: скрим и призрак драга тула.
//!
//! Хвост цепочки `render` вынесен как есть (`plan/100-refactor-250.md`);
//! обобщён по типу элемента, чтобы не зависеть от места в цепочке.

use crate::colors::rgba;
use crate::host_link::ShellEvent;
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn with_main_overlays<E>(&self, root: E, p: &'static Palette) -> E
    where
        E: gpui::Styled + gpui::ParentElement + gpui::prelude::Element,
    {
        root.when(
            self.palette_open
                || self.sov.quickopen_open
                || self.sov.fif_open
                || self.sov.ws_open
                || self.quick_pick.is_some()
                || self.modal.is_some(),
            |root| {
                // Клик по скриму закрывает инпут-оверлеи: «тёмная зона» =
                // main-окно (overlay физически вырезан регионом до бокса)
                let tx = self.tx.clone();
                root.child(gpui::deferred(
                    div()
                        .id("main-scrim")
                        .absolute()
                        .inset_0()
                        // ЛОВЕЦ КЛИКОВ, не затемнение: рисует скрим
                        // overlay-окно (оно поверх и накрывает вебвью).
                        // Пока красили оба, альфа складывалась — .6 + .6
                        // давало ≈ .84 (ревью ц.13).
                        .bg(gpui::Rgba {
                            r: 0.,
                            g: 0.,
                            b: 0.,
                            a: 0.,
                        })
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            let _ = tx.try_send(ShellEvent::CloseInputOverlays);
                        }),
                ))
            },
        )
        // Ghost перетаскиваемой плитки. `ActivityDragGhost.module.css`:
        // 28×28, radius-sm, фон accent 22% ПОВЕРХ bg-surface (непрозрачный
        // микс), рамка accent 50%, глиф accent-primary 18px, shadow
        // 0 4 14 /35%, opacity .92, `translate(-50%,-50%)` — центр строго
        // на курсоре. Вид от наличия цели НЕ зависит (в оригинале один
        // класс без вариантов).
        .when_some(
            self.tool_drag.as_ref().filter(|d| d.started).map(|d| {
                (
                    d.pos,
                    // Общий реестр, а не builtin-only: у contributed-тула
                    // ghost рисовался заглушкой circle-large (ревью ц.13)
                    crate::activity::lookup_any(&d.id)
                        .map(|(_, icon)| crate::activity::intern(&icon))
                        .unwrap_or("circle-large"),
                )
            }),
            |root, ((gx, gy), icon)| {
                const GHOST: f32 = 28.0;
                let bg = {
                    let a = rgba(p.accent_primary);
                    let b = rgba(p.bg_surface);
                    gpui::Rgba {
                        r: a.r * 0.22 + b.r * 0.78,
                        g: a.g * 0.22 + b.g * 0.78,
                        b: a.b * 0.22 + b.b * 0.78,
                        a: 1.0,
                    }
                };
                root.child(gpui::deferred(
                    div()
                        .absolute()
                        .left(px(gx - GHOST / 2.0))
                        .top(px(gy - GHOST / 2.0))
                        .w(px(GHOST))
                        .h(px(GHOST))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(m::RADIUS_SM))
                        .bg(bg)
                        .border_1()
                        .border_color({
                            let mut c = rgba(p.accent_primary);
                            c.a = 0.5;
                            c
                        })
                        .shadow(vec![gpui::BoxShadow {
                            color: gpui::Rgba {
                                r: 0.,
                                g: 0.,
                                b: 0.,
                                a: 0.35,
                            }
                            .into(),
                            offset: gpui::point(px(0.), px(4.)),
                            blur_radius: px(14.),
                            spread_radius: px(0.),
                        }])
                        .opacity(0.92)
                        // svg 18 (`DEFAULT_SIZE_PX`), codicon 16: `.ghost`
                        // правила для `.codicon` не задаёт (ревью ц.13)
                        .child(crate::ui::activity_bar::tool_glyph_split(
                            icon,
                            18.0,
                            16.0,
                            rgba(p.accent_primary),
                        )),
                ))
            },
        )
    }
}
