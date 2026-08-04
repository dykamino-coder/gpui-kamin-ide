//! Семплы триггеров: тосты, модалки, внешние тосты.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::ui::ds::buttons::{DsBtn, ds_btn};
use crate::ui::ds::state::next_id;
use crate::ui::modal::{Modal, ModalAction};
use crate::ui::toasts::Toast;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// `ToastTriggers` — 5 кнопок `.btnSecondary`, пушат тост в стек.
pub fn sample_toast_triggers(tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let rows: [(&'static str, &'static str, &'static str, bool); 5] = [
        ("ds-toast-info", "Push info", "info", false),
        ("ds-toast-success", "Push success", "success", false),
        ("ds-toast-warning", "Push warning", "warning", false),
        ("ds-toast-error", "Push error", "error", false),
        ("ds-toast-actions", "With actions", "info", true),
    ];
    let mut row = div().flex().flex_wrap().gap(px(m::SPACE_2));
    for (id, label, severity, with_actions) in rows {
        let tx_b = tx.clone();
        row = row.child(ds_btn(DsBtn::Secondary, id, label, p).on_mouse_down(
            gpui::MouseButton::Left,
            move |_, _, _| {
                let message = if with_actions {
                    "Pick an action."
                } else {
                    match severity {
                        "success" => "Sample success toast.",
                        "warning" => "Sample warning.",
                        "error" => "Sample error.",
                        _ => "Sample info toast.",
                    }
                };
                let _ = tx_b.try_send(ShellEvent::Toast(Toast {
                    id: format!("ds-{id}-{}", next_id()),
                    severity: severity.to_string(),
                    title: None,
                    message: message.to_string(),
                    actions: if with_actions {
                        vec!["Save".into(), "Discard".into()]
                    } else {
                        Vec::new()
                    },
                    sticky: with_actions,
                }));
            },
        ));
    }
    row.into_any_element()
}
/// `ModalTriggers` — Confirm / Confirm danger / Prompt.
pub fn sample_modal_triggers(tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let make = |kind: DsBtn, id: &'static str, label: &'static str, modal: Modal| {
        let tx_b = tx.clone();
        ds_btn(kind, id, label, p).on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx_b.try_send(ShellEvent::OpenModal(modal.clone()));
        })
    };
    div()
        .flex()
        .flex_wrap()
        .gap(px(m::SPACE_2))
        .child(make(
            DsBtn::Secondary,
            "ds-modal-confirm",
            "Confirm",
            Modal {
                title: "Sample confirm".into(),
                body: "This is a <code>ConfirmModal</code> demo.".into(),
                confirm_label: "Confirm".into(),
                danger: false,
                prompt: None,
                placeholder: None,
                validate: None,
                cancel_label: None,
                action: ModalAction::Noop,
            },
        ))
        .child(make(
            DsBtn::Danger,
            "ds-modal-danger",
            "Confirm danger",
            Modal {
                title: "Delete?".into(),
                body: "This action cannot be undone.".into(),
                confirm_label: "Delete".into(),
                danger: true,
                prompt: None,
                placeholder: None,
                validate: None,
                cancel_label: None,
                action: ModalAction::Noop,
            },
        ))
        .child(make(
            DsBtn::Secondary,
            "ds-modal-prompt",
            "Prompt",
            Modal {
                title: "Enter name".into(),
                body: "".into(),
                confirm_label: "OK".into(),
                danger: false,
                prompt: Some(String::new()),
                placeholder: Some("e.g. my-extension".into()),
                validate: None,
                cancel_label: None,
                action: ModalAction::Noop,
            },
        ))
        .into_any_element()
}
/// `ExternalToastTriggers` — 4 кнопки. Внешние (standalone-окно) тосты в
/// gpui-порте пока не реализованы, поэтому кнопки поднимают тот же тост
/// в стеке приложения; форма блока — 1:1.
pub fn sample_external_toast_triggers(tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let rows: [(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        bool,
        bool,
    ); 4] = [
        (
            "ds-ext-info",
            "Info (timed)",
            "info",
            "Build finished",
            "Sample with timer bar — hover to pause.",
            false,
            false,
        ),
        (
            "ds-ext-success",
            "Success (timed)",
            "success",
            "Sync complete",
            "All extensions synced — green accent + check glyph.",
            false,
            false,
        ),
        (
            "ds-ext-warning",
            "Warning (sticky)",
            "warning",
            "Approval pending",
            "Sticky — no auto-dismiss, no timer bar.",
            true,
            false,
        ),
        (
            "ds-ext-error",
            "Error (with actions)",
            "error",
            "Activation failed",
            "Pick what to do — Retry runs activate() again, Show log opens the Output channel.",
            true,
            true,
        ),
    ];
    let mut row = div().flex().flex_wrap().gap(px(m::SPACE_2));
    for (id, label, severity, title, message, sticky, actions) in rows {
        let tx_b = tx.clone();
        row = row.child(ds_btn(DsBtn::Secondary, id, label, p).on_mouse_down(
            gpui::MouseButton::Left,
            move |_, _, _| {
                // `window.kamin.externalToast.show({...})` — ВНЕШНИЙ тост
                // (отдельное окно), а не строка внутреннего стека
                let _ = tx_b.try_send(ShellEvent::ExternalToast(Toast {
                    id: format!("ds-{id}-{}", next_id()),
                    severity: severity.to_string(),
                    title: Some(title.to_string()),
                    message: message.to_string(),
                    actions: if actions {
                        vec!["Retry".into(), "Show log".into()]
                    } else {
                        Vec::new()
                    },
                    sticky,
                }));
            },
        ));
    }
    row.into_any_element()
}
