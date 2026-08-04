//! Контекст-меню веб-страницы В ТЕМЕ приложения (запрос: «под тему», а не
//! системное белое). Пункты приходят из модели CEF (`web/context_menu.rs`),
//! рисуются в overlay-слое как остальные поповеры: bg-surface, рамка 6 %,
//! ховер text-primary 10 % (`feedback_popover_surface`).

use crate::host_link::ShellEvent;
use crate::overlay::{dropdown_shadow, hit_area};
use gpui::prelude::*;
use gpui::px;
use kamin_metrics as m;

/// Состояние открытого меню страницы.
#[derive(Clone, Debug)]
pub struct WebMenuState {
    /// Вью, которому принадлежит меню (команда исполнится над ним).
    pub view: String,
    /// Пункты модели CEF: команда, подпись, доступность; `None` — разделитель.
    pub items: Vec<Option<(i32, String, bool)>>,
    /// Точка правого клика в ЛОГИЧЕСКИХ px окна.
    pub x: f32,
    pub y: f32,
}

const MENU_W: f32 = 200.0;
const ROW_H: f32 = 32.0;

pub fn web_menu(
    menu: &WebMenuState,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    vh: f32,
    p: &'static kamin_theme::Palette,
) -> gpui::AnyElement {
    use crate::colors::rgba;

    let rows = menu.items.iter().flatten().count();
    let seps = menu.items.len() - rows;
    let est_h = ROW_H * rows as f32 + 9.0 * seps as f32 + m::SPACE_1 * 2.0 + 2.0;
    // Якорь — как у Chromium: левый верх меню в курсоре, с клэмпом в окно и
    // флипом вверх, когда снизу не помещается.
    let x = menu.x.clamp(8.0, (vw - MENU_W - 8.0).max(8.0));
    let y = if menu.y + est_h > vh - 8.0 && menu.y - est_h >= 8.0 {
        menu.y - est_h
    } else {
        menu.y.min((vh - est_h - 8.0).max(8.0))
    };
    let tint = |mut c: gpui::Rgba, a: f32| {
        c.a = a;
        c
    };
    let hover_bg = tint(rgba(p.text_primary), 0.10);

    let mut boxed = gpui::div()
        .id("web-ctx-menu")
        .occlude()
        .absolute()
        .left(px(x))
        .top(px(y))
        .min_w(px(MENU_W))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .max_h(px((vh - 16.0).max(64.0)))
        .shadow(dropdown_shadow())
        .child(hit_area())
        .child(crate::probe::registry::probe_area("web-ctx-menu"));

    for (i, item) in menu.items.iter().enumerate() {
        match item {
            None => {
                boxed = boxed.child(
                    gpui::div()
                        .my(px(m::SPACE_1))
                        .h(px(1.0))
                        .bg(tint(rgba(p.text_primary), 0.08)),
                );
            }
            Some((cmd, label, enabled)) => {
                let row = gpui::div()
                    .id(gpui::SharedString::from(format!("web-ctx-{i}")))
                    .flex()
                    .items_center()
                    .px(px(m::SPACE_3))
                    .py(px(m::SPACE_2))
                    .rounded(px(m::RADIUS_SM))
                    .text_size(px(m::FS_SM))
                    .child(label.clone());
                boxed = boxed.child(if *enabled {
                    let tx = tx.clone();
                    let view = menu.view.clone();
                    let cmd = *cmd;
                    row.text_color(rgba(p.text_primary))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover_bg))
                        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                            let _ = tx.try_send(ShellEvent::WebMenuCmd(view.clone(), cmd));
                        })
                } else {
                    row.text_color(tint(rgba(p.text_primary), 0.35))
                });
            }
        }
    }
    boxed.into_any_element()
}
