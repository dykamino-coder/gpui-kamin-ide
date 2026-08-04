//! Семплы выпадающих меню и чекбокс-меню.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::ui::design_samples::COLOR_MODE;
use crate::ui::design_samples::DEVICE_DESKTOP;
use crate::ui::design_samples::LIGHTBULB;
use crate::ui::design_samples::{CHECK, CHEVRON_DOWN};
use crate::ui::ds::buttons::menu_group_label;
use crate::ui::ds::buttons::{DsBtn, ds_btn};
use crate::ui::ds::state::DesignAction;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// `DropdownRow` — форма ThemeQuickToggle: триггер `.btnSecondary`
/// (glyph + «Theme» + chevron) и абсолютное меню под ним:
/// min-w 220, bg-mantle, r-md, shadow-dropdown, padding 4, gap 1.
pub fn sample_dropdown(
    open: bool,
    picked: &str,
    // Светлая тема: у выбранного пункта отдельное состояние
    light: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let items: [(
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    ); 3] = [
        ("dark", COLOR_MODE, "Dark", Some("default")),
        ("light", LIGHTBULB, "Light", None),
        ("system", DEVICE_DESKTOP, "System", None),
    ];
    let tx_t = tx.clone();
    let trigger = ds_btn(DsBtn::Secondary, "ds-dropdown-trigger", "", p)
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .child(codicon(COLOR_MODE, m::FS_MD))
        .child("Theme")
        .child(codicon(CHEVRON_DOWN, m::FS_MD))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx_t.try_send(ShellEvent::Cz(CzEvent::DesignSample(
                DesignAction::ToggleDropdown,
            )));
        });

    let mut anchor = div().relative().flex().child(trigger);
    if open {
        let mut menu = div()
            .absolute()
            // `top: calc(100% + 4px)` — низ триггера + 4
            .top(gpui::relative(1.0))
            .mt(px(m::SPACE_1))
            .left(px(0.))
            // `min-width: 220` + `.dropdownItem { width: 100% }`: у нас меню
            // абсолютное, и процент ребёнка taffy не резолвит — заливка
            // выбранного пункта была короче меню на 58 px, а hint с галкой
            // не прижимались вправо. Даём ЯВНУЮ ширину (ревью ц.18).
            // `.dropdownMenu { min-width: 220px }` — жёсткая ширина не
            // давала меню расти под длинный пункт (ревью ц.35)
            .min_w(px(220.0))
            .flex()
            .flex_col()
            .gap(px(1.0))
            .p(px(m::SPACE_1))
            .rounded(px(m::RADIUS_MD))
            .bg(rgba(p.bg_mantle))
            .shadow(crate::ui::shadows::dropdown())
            .child(menu_group_label("Built-in", p));
        for (id, glyph, label, hint) in items {
            let is_picked = picked == id;
            let tx_i = tx.clone();
            let hover_bg = tint(rgba(p.bg_surface), 0.6);
            let mut item = div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .w_full()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                // `.dropdownItem` кегль кодикона НЕ переопределяет → база
                // `.codicon { font-size: 16px }` (skeleton.css), не 13
                .child(codicon(glyph, 16.0))
                .child(div().flex_1().child(label));
            if let Some(h) = hint {
                // `[data-theme=light] .dropdownItemPicked .dropdownItemHint`
                // — hint выбранного тоже перекрашивается (ревью ц.18)
                let hint_color = if is_picked && light {
                    rgba(p.accent_action_fg)
                } else {
                    rgba(p.text_muted)
                };
                item = item.child(
                    div()
                        .font_family(crate::ui::design_panel::MONO)
                        .text_size(px(m::FS_XS))
                        .text_color(hint_color)
                        .child(h),
                );
            }
            if is_picked && light {
                // `[data-theme=light] .dropdownItemPicked` — сплошная заливка
                // accent, а текст, глиф, hint и галка перекрашены в
                // accent-action-fg; вес 600 (ревью ц.15)
                item = item
                    .bg(rgba(p.accent_primary))
                    .text_color(rgba(p.accent_action_fg))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(codicon(CHECK, 16.0));
            } else if is_picked {
                item = item
                    .bg(tint(rgba(p.accent_primary), 0.12))
                    .text_color(rgba(p.accent_primary))
                    .child(codicon(CHECK, 16.0));
            }
            // `.dropdownItem:hover` (0,2,0) перебивает `.dropdownItemPicked`
            // (0,1,0) — ховер работает и на выбранном пункте
            // `.dropdownItem:hover` (0,2,0) бьёт `.dropdownItemPicked`
            // (0,1,0) — тёмный выбранный пункт на ховере серый. Но
            // `[data-theme=light] .dropdownItemPicked` (`:356-360`) — тоже
            // (0,2,0) и объявлен ПОЗЖЕ, значит в светлой теме выбранный
            // пункт под курсором ОСТАЁТСЯ accent-заливкой (ревью ц.25:
            // подпись выцветала на сером фоне)
            if !(is_picked && light) {
                item = item.hover(move |s| s.bg(hover_bg));
            }
            let ev_id = id.to_string();
            let item = item.on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let _ = tx_i.try_send(ShellEvent::Cz(CzEvent::DesignSample(DesignAction::Pick(
                    ev_id.clone(),
                ))));
            });
            // Пункты меню оригинала — `<button>`, значит таб-стопы с
            // `button:focus-visible` (`theme/global.css:38-43`)
            menu = menu.child(crate::ui::focus_ring::focusable(
                item,
                id,
                m::RADIUS_SM,
                rgba(p.accent_primary),
            ));
        }
        // `z-index: var(--z-dropdown)` — в gpui порядок отрисовки задаётся
        // deferred-приоритетом, иначе следующие блоки семплов рисуются ПОВЕРХ
        anchor = anchor.child(gpui::deferred(menu).with_priority(60));
    }
    anchor.into_any_element()
}
/// `CheckboxDropdownRow` — рецепт LayoutToggles: статично встроенное меню
/// (position static, без тени), клик по пункту НЕ закрывает меню.
pub fn sample_checkbox_dropdown(
    checks: [bool; 3],
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let labels = ["Option A", "Option B", "Option C"];
    let mut menu = div()
        .min_w(px(220.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .child(menu_group_label("Sample", p));
    for (i, label) in labels.iter().enumerate() {
        let on = checks[i];
        let tx_i = tx.clone();
        let hover_bg = tint(rgba(p.text_primary), 0.1);
        // `.check` 16×16, r 3, рамка bg-overlay; включённый — заливка accent
        let mut check = div()
            .w(px(16.0))
            .h(px(16.0))
            .flex()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(3.0))
            .border_1()
            .border_color(rgba(p.bg_overlay));
        if on {
            check = check
                .bg(rgba(p.accent_primary))
                .border_color(rgba(p.accent_primary))
                .text_color(rgba(p.accent_action_fg))
                .child(codicon(CHECK, m::FS_SM));
        }
        menu = menu.child(
            div()
                .id(*label)
                .flex()
                .items_center()
                .gap(px(m::SPACE_2))
                .w_full()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg))
                .child(check)
                .child(div().flex_1().child(*label))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                    let _ = tx_i.try_send(ShellEvent::Cz(CzEvent::DesignSample(
                        DesignAction::ToggleCheck(i),
                    )));
                }),
        );
    }
    menu.into_any_element()
}
