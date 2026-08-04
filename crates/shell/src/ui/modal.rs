//! ConfirmModal 1:1: скрим overlay-deep, диалог bg-primary min320 max480
//! padding 20 radius-md shadow-modal; заголовок fs-md 600, тело fs-sm
//! text-secondary; кнопки Cancel (бордер) / Confirm (accent-action,
//! danger=accent-red). Esc/бэкдроп-клик закрывают. (Переименование сессии —
//! inline в строке, НЕ модалка — см. sessions_list.)

pub use crate::ui::modal_model::{Modal, ModalAction};
use crate::ui::modal_parts::dialog_button_bg;
use crate::ui::modal_parts::{body_text, dialog_button};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use gpui_component::Sizable as _;
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;

// Компонент дизайн-системы: аргументы — его пропсы.
#[allow(clippy::too_many_arguments)]
/// Рендер ConfirmModal. `input` — Some для prompt-режима (создаёт вызывающий,
/// т.к. InputState требует Window того окна, где рендерится модалка).
pub fn render_modal(
    modal: &Modal,
    input: Option<&gpui::Entity<gpui_component::input::InputState>>,
    p: &Palette,
    // Текст ошибки валидации: считает вызывающий (у него есть cx, чтобы
    // прочитать значение инпута). None = валидно либо валидатора нет.
    error: Option<SharedString>,
    // Инпут в фокусе? `.input:focus { border-color: accent-primary }` —
    // без фокуса рамка `--bg-surface` (ревью ц.20/22)
    input_focused: bool,
    // Прошло с открытия, мс — `fadeIn 0.12s ease-out`
    // (`ConfirmModal.module.css:9,12-15`)
    age_ms: u128,
    // Базовый шрифт окна: пер-ранные стили тела строим от него
    // (`HighlightStyle` семейством не управляет, `TextRun` — управляет)
    base_font: gpui::Font,
    on_cancel: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
    on_confirm: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let title = modal.title.clone();
    let body_el = div()
        .mb(px(m::SPACE_4))
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_secondary))
        .line_height(px(m::FS_SM * 1.3))
        .child(body_text(&modal.body, p, base_font))
        .into_any_element();
    let confirm_label = modal.confirm_label.clone();
    let cancel_label = modal
        .cancel_label
        .clone()
        .unwrap_or_else(|| SharedString::from("Cancel"));
    let danger = modal.danger;

    // .confirmBtn: action/hover; danger: red/maroon (ConfirmModal.module.css)
    let confirm_bg = if danger {
        rgba(p.accent_red)
    } else {
        rgba(p.accent_action)
    };
    let confirm_hover = if danger {
        rgba(p.accent_maroon)
    } else {
        rgba(p.accent_action_hover)
    };
    // Точка нажатия на скриме: закрытие требует, чтобы и нажатие, и
    // отпускание были ВНЕ диалога (`target === currentTarget` оригинала)
    let scrim_down: std::rc::Rc<std::cell::Cell<Option<(f32, f32)>>> =
        std::rc::Rc::new(std::cell::Cell::new(None));
    // on_cancel дёргается дважды (скрим + кнопка Cancel) — общий Rc
    let on_cancel = std::rc::Rc::new(on_cancel);
    let on_cancel_scrim = on_cancel.clone();
    // Enter в prompt-режиме = сабмит (PromptModal 1:1)
    let on_confirm = std::rc::Rc::new(on_confirm);
    let on_confirm_enter = on_confirm.clone();
    // PromptModal шире ConfirmModal: 360/520 против 320/480
    let is_prompt = input.is_some();
    let invalid = error.is_some();
    let (dlg_min_w, dlg_max_w) = if is_prompt {
        (360.0, 520.0)
    } else {
        (320.0, 480.0)
    };

    // Скрим: клик по бэкдропу закрывает
    // `fadeIn 0.12s ease-out`: 0 → 1 по кубической выходной кривой
    const FADE_MS: f32 = 120.0;
    let t = (age_ms as f32 / FADE_MS).clamp(0.0, 1.0);
    let fade = 1.0 - (1.0 - t).powi(3);
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .when(fade < 1.0, |d| d.opacity(fade))
        .flex()
        .items_center()
        .justify_center()
        .bg(crate::ui::scrim::deep())
        // `onClick` с проверкой `target === currentTarget`
        // (`ConfirmModal.tsx:76`): закрываем на ОТПУСКАНИИ и только если и
        // нажатие, и отпускание были вне коробки диалога. Раньше висело на
        // mouse-down, и «нажал на скриме, отпустил в диалоге» закрывало
        // модалку (ревью ц.26)
        .id("modal-scrim")
        .on_mouse_down(gpui::MouseButton::Left, {
            let down = scrim_down.clone();
            move |ev: &gpui::MouseDownEvent, _, _| {
                down.set(Some((f32::from(ev.position.x), f32::from(ev.position.y))));
            }
        })
        .on_mouse_up(gpui::MouseButton::Left, {
            let down = scrim_down.clone();
            move |ev: &gpui::MouseUpEvent, w, cx| {
                let started_outside = down.take().is_some();
                let inside_dialog = crate::probe::registry::bounds_of("modal-dialog").is_some_and(
                    |[x, y, bw, bh]| {
                        let (ux, uy) = (f32::from(ev.position.x), f32::from(ev.position.y));
                        ux >= x && ux <= x + bw && uy >= y && uy <= y + bh
                    },
                );
                if started_outside && !inside_dialog {
                    on_cancel_scrim(w, cx);
                }
            }
        })
        .child(
            // Диалог — клик внутри не закрывает (stop_propagation)
            div()
                .relative()
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Бокс диалога для проверки `target === currentTarget`
                .child(crate::probe::registry::probe_area("modal-dialog"))
                // Видимый регион overlay-окна (ввод даёт input_area на всё окно)
                .child(crate::overlay::region_area())
                .min_w(px(dlg_min_w))
                .max_w(px(dlg_max_w))
                .p(px(m::SPACE_5))
                .rounded(px(m::RADIUS_MD))
                .bg(rgba(p.bg_primary))
                .border_1()
                .border_color(rgba(p.bg_surface))
                .shadow(crate::ui::shadows::modal())
                .child(
                    div()
                        .mb(px(m::SPACE_3))
                        .text_size(px(m::FS_MD))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgba(p.text_primary))
                        .child(title),
                )
                // В prompt-режиме абзаца между заголовком и полем НЕТ
                // (`PromptModal.tsx:77-87`); он есть только у ConfirmModal
                .when(!is_prompt, |d| d.child(body_el))
                .when_some(input, |d, input| {
                    d.child(
                        // `.input`: padding 8/12, bg-base, рамка bg-surface,
                        // radius-sm, fs-md. Фокус — accent-primary; поле
                        // автофокусится при открытии, так что это состояние
                        // по умолчанию. `.invalid` — accent-red.
                        div()
                            .px(px(m::SPACE_3))
                            .py(px(m::SPACE_2))
                            .rounded(px(m::RADIUS_SM))
                            .bg(rgba(p.bg_base))
                            .border_1()
                            .border_color(rgba(if invalid {
                                p.accent_red
                            } else if input_focused {
                                p.accent_primary
                            } else {
                                // `.input { border: 1px solid var(--bg-surface) }`
                                p.bg_surface
                            }))
                            .on_key_down(move |ev: &gpui::KeyDownEvent, w, cx| {
                                if ev.keystroke.key.as_str() == "enter" {
                                    cx.stop_propagation();
                                    on_confirm_enter(w, cx);
                                }
                            })
                            .child(
                                gpui_component::input::Input::new(input)
                                    .appearance(false)
                                    // Input берёт кегль из своего Size (×0.875)
                                    .with_size(gpui_component::Size::Size(px(m::FS_MD / 0.875))),
                            ),
                    )
                })
                // `.error` — только когда валидатор вернул текст
                .when_some(error.clone(), |d, err| {
                    d.child(
                        div()
                            .mt(px(m::SPACE_2))
                            .text_size(px(m::FS_XS))
                            .text_color(rgba(p.accent_red))
                            .child(err),
                    )
                })
                .child(
                    div()
                        .flex()
                        .gap(px(m::SPACE_2))
                        .justify_end()
                        // `margin-top: space-4` есть ТОЛЬКО у
                        // `PromptModal.actions`; у ConfirmModal отступ даёт
                        // `.body { margin-bottom: space-4 }`, и добавочный
                        // mt давал 32 вместо 16 (ревью ц.14)
                        .when(modal.prompt.is_some(), |d| d.mt(px(m::SPACE_4)))
                        .child(dialog_button("modal-cancel", cancel_label, false, p, {
                            move |w, cx| on_cancel(w, cx)
                        }))
                        .child(dialog_button_bg(
                            "modal-confirm",
                            confirm_label,
                            confirm_bg,
                            confirm_hover,
                            rgba(p.accent_action_fg),
                            // `.confirmBtn:disabled` — opacity .5 и сабмита нет
                            invalid,
                            rgba(p.accent_primary),
                            move |w, cx| on_confirm(w, cx),
                        )),
                ),
        )
        .into_any_element()
}
