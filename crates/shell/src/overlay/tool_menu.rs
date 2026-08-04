//! RMB-меню таба тула: Hide (unpin) + перенос в другой слот.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use super::{dropdown_shadow, hit_area};
use crate::host_link::ShellEvent;
use gpui::prelude::*;

/// RMB-меню таба тула: Hide (unpin) + Move to <слот> (все прочие слоты).
#[allow(clippy::too_many_arguments)]
pub(super) fn tool_tab_menu(
    slot: crate::activity::PanelSlot,
    id: String,
    x: f32,
    y: f32,
    sub_open: bool,
    vw: f32,
    vh: f32,
    tx: &smol::channel::Sender<ShellEvent>,
    p: &'static kamin_theme::Palette,
) -> gpui::AnyElement {
    use crate::colors::rgba;
    use gpui::px;
    use kamin_metrics as m;

    // ActivityContextMenu 1:1: min-w 180, p space-1, gap 1px; пункты:
    // gap space-2, padding space-2/space-3, radius-sm, text-PRIMARY, fs-sm,
    // hover text-primary 10%; сабменю — отдельный бокс справа с PanelIcon.
    const MENU_W: f32 = 180.0;
    // Пункт: py 8×2 + max(глиф 16, текст fs-sm при lh 1.169 = 14.03) = 32
    // (ревью ц.13: 35.2 было взято из несуществующего здесь lh 1.6);
    // контейнер — p 4×2 + рамка 1×2 + gap 1 между пунктами → меню 75.
    const ROW_H: f32 = 32.0;
    let menu_h = |rows: usize| {
        ROW_H * rows as f32 + (rows.saturating_sub(1)) as f32 + m::SPACE_1 * 2.0 + 2.0
    };
    // Строка САБМЕНЮ ниже на 2: её иконка — `PanelIcon` 14×12, а не codicon
    // 16, поэтому высота = py 8×2 + текст 14.03 ≈ 30 (ревью ц.17: центровка
    // промахивалась на 4.8 px из-за общей ROW_H)
    const SUB_ROW_H: f32 = 30.0;
    let sub_menu_h = |rows: usize| {
        SUB_ROW_H * rows as f32 + (rows.saturating_sub(1)) as f32 + m::SPACE_1 * 2.0 + 2.0
    };
    let est_h = menu_h(2);
    // Оригинал даёт `clampToViewport` НУЛЕВОЙ якорь в курсоре при
    // `side: "bottom"` → меню центрируется по курсору `left = x − w/2`
    // (`ActivityContextMenu.tsx:100-105` + `clamp-popup.ts:100`);
    // у нас левый край стоял НА курсоре — расхождение до 90px (ревью ц.15)
    let x = (x - MENU_W / 2.0).clamp(8.0, (vw - MENU_W - 8.0).max(8.0));
    // Флип вверх, если снизу не помещается, а сверху помещается
    let y = if y + est_h > vh - 8.0 && y - est_h >= 8.0 {
        y - est_h
    } else {
        y.min(vh - est_h - 8.0).max(8.0)
    };
    let tint = |mut c: gpui::Rgba, a: f32| {
        c.a = a;
        c
    };
    let box_style = |d: gpui::Stateful<gpui::Div>| {
        d.occlude()
            .absolute()
            .min_w(px(MENU_W))
            .flex()
            .flex_col()
            .gap(px(1.0))
            .p(px(m::SPACE_1))
            .rounded(px(m::RADIUS_MD))
            .bg(rgba(p.bg_surface))
            .border_1()
            .border_color(tint(rgba(p.text_primary), 0.06))
            // `max-height: calc(100vh - 16px); max-width: calc(100vw - 16px)`
            .max_h(px((vh - 16.0).max(64.0)))
            .max_w(px((vw - 16.0).max(180.0)))
            .overflow_y_scroll()
            // `--shadow-dropdown` — как у всех прочих дропдаунов
            // (единственное меню, где тени не было)
            .shadow(dropdown_shadow())
            .child(hit_area())
    };
    let hover_bg = tint(rgba(p.text_primary), 0.10);

    // ── корневое меню: Hide + Move to ▸
    let hide_tx = tx.clone();
    let hide_slot = slot;
    let hide_id = id.clone();
    let root_menu = box_style(gpui::div().id("tool-tab-menu"))
        .left(px(x))
        .top(px(y))
        .child(
            gpui::div()
                .id("ttm-hide")
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .on_hover({
                    let tx_close_sub = tx.clone();
                    move |hovered: &bool, _, _| {
                        // Курсор ушёл с «Move to» на «Hide» → сабменю гаснет
                        if *hovered {
                            let _ = tx_close_sub.try_send(ShellEvent::ToolMenuSub(false));
                        }
                    }
                })
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = hide_tx.try_send(ShellEvent::UnpinTool(hide_slot, hide_id.clone()));
                    let _ = hide_tx.try_send(ShellEvent::CloseToolTabMenu);
                })
                // `.item :global(.codicon)` в модуле НЕ задан → база
                // `.codicon { font-size: 16px }`; цвет наследуется от строки
                // Бокс = кегль (`.codicon { font-size: 16px; line-height: 1 }`):
                // без него line-box 18.7 растягивал строку меню (ревью ц.13)
                .child(crate::ui::icon::codicon("\u{eae7}", 16.0)) // eye-closed
                .child(gpui::div().flex_1().child("Hide")),
        )
        .child({
            // Move to ▸ — ховер открывает сабменю; открытое = accent 16%
            let tx_sub = tx.clone();
            let mut row = gpui::div()
                .id("ttm-moveto")
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                // `.itemMoveTo[aria-expanded="true"]` стоит ПОСЛЕ `.item:hover`
                // при равной специфичности (0,2,0) и выигрывает: с открытым
                // сабменю строка держит accent 16 % даже под курсором
                // (ревью ц.21 — у нас ховер перекрывал accent)
                .when(!sub_open, |r| r.hover(move |s| s.bg(hover_bg)))
                .on_hover(move |hovered: &bool, _, _| {
                    if *hovered {
                        let _ = tx_sub.try_send(ShellEvent::ToolMenuSub(true));
                    }
                })
                .child(crate::ui::icon::codicon("\u{ea9c}", 16.0)) // arrow-right
                .child(gpui::div().flex_1().child("Move to"))
                .child(
                    // `.chevron { font-size: 12px }` — единственное место, где
                    // модуль кегль задаёт
                    // `.chevron{font-size:12px}` стоит на ТОМ ЖЕ элементе,
                    // что `.codicon` (0,1,0) → проигрывает базе (0,2,0):
                    // эффективный кегль 16. Цвет `--text-muted` при этом
                    // побеждает — в шорткате `font` цвета нет (ревью ц.14)
                    crate::ui::icon::codicon(crate::ui::icon::CHEVRON_RIGHT, 16.0)
                        .text_color(rgba(p.text_muted)),
                );
            if sub_open {
                row = row.bg(tint(rgba(p.accent_primary), 0.16));
            }
            row
        });

    // ── сабменю: другие панели с PanelIcon (порядок оригинала)
    let entries: [crate::overlay::tool_submenu::Entry; 6] = [
        (
            crate::activity::PanelSlot::Sidebar,
            crate::ui::panel_placeholder::SlotIcon::Main,
            "Sidebar",
        ),
        (
            crate::activity::PanelSlot::Main,
            crate::ui::panel_placeholder::SlotIcon::Main,
            "Left",
        ),
        (
            crate::activity::PanelSlot::MainBottom,
            crate::ui::panel_placeholder::SlotIcon::MainBottom,
            "Left Bottom",
        ),
        (
            crate::activity::PanelSlot::CentralBottom,
            crate::ui::panel_placeholder::SlotIcon::CenterBottom,
            "Center Bottom",
        ),
        (
            crate::activity::PanelSlot::RightTop,
            crate::ui::panel_placeholder::SlotIcon::RightTop,
            "Right",
        ),
        (
            crate::activity::PanelSlot::RightBottom,
            crate::ui::panel_placeholder::SlotIcon::RightBottom,
            "Right Bottom",
        ),
    ];
    let mut layer = gpui::div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(root_menu);
    if sub_open {
        layer = crate::overlay::tool_submenu::submenu(
            layer, slot, &id, &entries, MENU_W, ROW_H, sub_menu_h, box_style, hover_bg, x, y, vw,
            vh, tx, p,
        );
    }
    layer.into_any_element()
}
