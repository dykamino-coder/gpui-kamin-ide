//! Каскад «Open In ▸»: отдельный бокс справа от меню (влево при нехватке).
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host::events::ShellEvent;
use crate::ui::fmenu::items::{FA_FOLDER_OPEN, FA_TERMINAL, FA_WINDOW, MARGIN, SUB_W, item};
use crate::ui::fmenu::model::FileMenu;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_in(
    layer: gpui::Div,
    menu: &FileMenu,
    is_dir: bool,
    x: f32,
    y: f32,
    menu_w: f32,
    viewport_w: f32,
    viewport_h: f32,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> gpui::Div {
    let mut layer = layer;
    if menu.open_in {
        // Якорь каскада — правый край ИЗМЕРЕННОГО меню, а не минимальной
        // ширины 180 (ревью ц.15)
        // Якорь — ИЗМЕРЕННЫЙ rect строки «Open In» (`FileContextMenu.tsx:76`
        // берёт `e.currentTarget.getBoundingClientRect()`), а не вывод из
        // коробки меню: вывод промахивался на единицы px (ревью ц.23).
        // Строка живёт кадром раньше каскада, так что замер всегда есть;
        // оценка — только на самый первый кадр меню.
        let [row_x, row_y, row_w, row_h] =
            crate::probe::registry::bounds_of("file-menu-openin-row").unwrap_or([
                x + 5.0,
                y + 5.0,
                menu_w - 8.0,
                31.6,
            ]);
        // `side: "right", offset: 2` (`clamp-popup.ts:103-106`)
        let sub_right = row_x + row_w + 2.0;
        // Флип — ТОЛЬКО если слева реально влезает (`clamp-popup.ts:92-94`);
        // главную ось оригинал не клампит, каскад свисает.
        let sub_left = row_x - 2.0 - SUB_W;
        let sub_x = if sub_right + SUB_W > viewport_w - MARGIN && sub_left >= MARGIN {
            sub_left
        } else {
            sub_right
        };
        // `side: "right"` центрирует каскад по строке-якорю:
        // `top = row.top + row.height/2 − sub.height/2` (`clamp-popup.ts:103`)
        let sub_h = crate::probe::registry::bounds_of("file-submenu")
            .map(|[_, _, _, h]| h + 2.0)
            .unwrap_or(120.0);
        let sub_y = (row_y + row_h / 2.0 - sub_h / 2.0)
            .min(viewport_h - sub_h - MARGIN)
            .max(MARGIN);
        let term_dir = if is_dir {
            menu.path.clone()
        } else {
            std::path::Path::new(&menu.path)
                .parent()
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| menu.path.clone())
        };
        let mut sub = div()
            .id("file-menu-sub")
            .occlude()
            .absolute()
            .left(px(sub_x))
            .top(px(sub_y))
            // Замер собственной коробки для якоря следующего кадра (ц.21)
            .child(crate::probe::registry::probe_area("file-submenu"))
            .min_w(px(SUB_W))
            // `.menu { max-width: calc(100vw − 16px); overflow-y: auto }` —
            // длинный каскад скроллится, а не режется (ревью ц.21)
            .max_w(px((viewport_w - 16.0).max(SUB_W)))
            .max_h(px((viewport_h - 16.0).max(80.0)))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(1.0))
            .p(px(m::SPACE_1))
            .rounded(px(m::RADIUS_MD))
            .bg(rgba(p.bg_surface))
            .border_1()
            .border_color(tint(rgba(p.text_primary), 0.06))
            .shadow(crate::overlay::dropdown_shadow())
            .child(crate::overlay::hit_area());
        sub = sub.child(item(
            "fm-oi-reveal",
            FA_FOLDER_OPEN,
            "Reveal in File Explorer",
            false,
            p,
            {
                let tx = tx.clone();
                let path = menu.path.clone();
                move || {
                    let win_path = path.replace('/', "\\");
                    let mut cmd = std::process::Command::new("explorer.exe");
                    if is_dir {
                        cmd.arg(&win_path);
                    } else {
                        cmd.arg(format!("/select,{win_path}"));
                    }
                    let _ = cmd.spawn();
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                }
            },
        ));
        if is_dir {
            sub = sub.child(item(
                "fm-oi-terminal",
                FA_TERMINAL,
                "Open in Terminal",
                false,
                p,
                {
                    let tx = tx.clone();
                    move || {
                        let _ =
                            tx.try_send(ShellEvent::Ed(EdEvent::OpenInTerminal(term_dir.clone())));
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                    }
                },
            ));
        } else {
            sub = sub.child(item(
                "fm-oi-assoc",
                FA_WINDOW,
                "Open in Associated Application",
                false,
                p,
                {
                    let tx = tx.clone();
                    let path = menu.path.clone();
                    move || {
                        let win_path = path.replace('/', "\\");
                        let _ = std::process::Command::new("cmd")
                            .args(["/c", "start", "", &win_path])
                            .spawn();
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                    }
                },
            ));
        }
        layer = layer.child(sub);
    }
    layer
}
