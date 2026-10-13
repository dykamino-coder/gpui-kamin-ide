//! Меню-пилюли действий: сессия, проект, оверлейная отрисовка.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::colors::tint;
use crate::host_link::{self, ShellEvent};
use crate::ui::icon::{ADD, codicon};
use crate::ui::sessions::glyphs::{DISCONNECT, EDIT, FOLDER_OPENED, TRASH};
use crate::ui::sessions::pill::{pill_btn, pill_wrap};
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_model::Session;
use kamin_theme::Palette;
use serde_json::json;
use smol::channel::Sender;

/// Hover-пилюля действий сессии: rename / disconnect (open) / delete.
pub fn session_actions_pill(s: &Session, tx: &Sender<ShellEvent>, p: &Palette) -> AnyElement {
    let mut pill = pill_wrap(format!("pill-s-{}", s.id), p).child(pill_btn(
        format!("rn-{}", s.id),
        EDIT,
        "Rename",
        Some(rgba(p.accent_primary)),
        false,
        // `.popAction > i{13px}` — это (0,1,1), вендорная база
        // `.codicon[class*=codicon-]` (0,2,0) сильнее: эффективно 16 (ревью ц.14)
        16.0,
        p,
        {
            let tx = tx.clone();
            let id = s.id.clone();
            move || {
                let _ = tx.try_send(ShellEvent::BeginRename(id.clone()));
            }
        },
    ));
    if s.open {
        pill = pill.child(pill_btn(
            format!("dc-{}", s.id),
            DISCONNECT,
            "Disconnect (free from memory)",
            Some(rgba(p.accent_blue)),
            false,
            16.0,
            p,
            {
                let id = s.id.clone();
                let tx = tx.clone();
                move || {
                    let _ = tx.try_send(ShellEvent::LocalSessionClosed(id.clone()));
                    let id = id.clone();
                    std::thread::spawn(move || {
                        if let Some(c) = host_link::client() {
                            let _ = c.request("kamin:sessions:deactivate", vec![json!(id)]);
                        }
                    });
                }
            },
        ));
    }
    pill.child(pill_btn(
        format!("del-{}", s.id),
        TRASH,
        "Delete session",
        Some(rgba(p.accent_red)),
        false,
        16.0,
        p,
        {
            let tx = tx.clone();
            let id = s.id.clone();
            let name = s.name.clone();
            move || {
                let _ = tx.try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                    title: "Delete session?".into(),
                    body: format!(
                        "Session <strong>{name}</strong> will be removed. This cannot be undone."
                    )
                    .into(),
                    confirm_label: "Delete".into(),
                    danger: true,
                    prompt: None,
                    placeholder: None,
                    validate: None,
                    cancel_label: None,
                    action: crate::ui::modal::ModalAction::DeleteSession(id.clone()),
                }));
            }
        },
    ))
    .into_any_element()
}
/// Полноширинная action-строка (New session (folder…) / No folder session).
/// Иконки и подписи — те же, что в меню «+» титлбара (решение владельца,
/// 2026-10-13): codicon folder-opened и circle-large-outline.
pub(crate) fn action_row(
    label: &'static str,
    glyph: &'static str,
    p: &Palette,
    on_click: impl Fn() + 'static,
) -> AnyElement {
    // `data-tooltip` кнопок (`SessionsMode.tsx:14,17`) — их не было
    let tip = if label == "No folder session" {
        "Start without a folder"
    } else {
        "Pick a folder, then start a session"
    };
    let hover_bg = tint(rgba(p.bg_surface), 0.6);
    let hf = rgba(p.text_primary);
    let group: SharedString = format!("act-{label}").into();
    div()
        .id(label)
        .group(group.clone())
        .flex()
        .items_center()
        .gap(px(10.0))
        .w_full()
        .px(px(m::SPACE_2))
        .py(px(6.0))
        .rounded(px(m::RADIUS_SM))
        .tooltip(crate::ui::tooltip::tooltip(tip))
        .text_size(px(m::FS_MD))
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(move |s| s.bg(hover_bg).text_color(hf))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| on_click())
        .child(
            // `.action:hover > i { color: var(--text-primary) }`
            codicon(glyph, 16.0)
                .w(px(20.0))
                .text_color(rgba(p.text_muted))
                .group_hover(group, move |s| s.text_color(hf)),
        )
        .child(label)
        .into_any_element()
}
/// Ширина пилюли проекта для клампа: кнопки по 24, gap 2, p 3×2, border 1×2.
pub fn project_actions_pill_width(has_folder: bool) -> f32 {
    let n = if has_folder { 3.0 } else { 2.0 };
    n * 24.0 + (n - 1.0) * 2.0 + 8.0
}
/// Hover-пилюля действий группы проекта: add-session + reveal-folder (если
/// у проекта есть папка) + delete-project.
pub fn project_actions_pill(
    pid: &str,
    name: &str,
    folder_path: Option<&str>,
    count: usize,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let mut pill = pill_wrap(format!("pill-p-{pid}"), p).child(pill_btn(
        format!("padd-{pid}"),
        ADD,
        "New session here",
        Some(rgba(p.accent_primary)),
        false,
        14.0,
        p,
        {
            let pid = pid.to_string();
            move || {
                let pid = pid.clone();
                std::thread::spawn(move || {
                    if let Some(c) = host_link::client() {
                        let _ = c.request("kamin:sessions:newSession", vec![json!(pid)]);
                    }
                });
            }
        },
    ));
    if let Some(folder) = folder_path.filter(|f| !f.is_empty()) {
        // Открыть папку проекта в Проводнике (решение владельца, 2026-10-13)
        let win_path = folder.replace('/', "\\");
        pill = pill.child(pill_btn(
            format!("pdir-{pid}"),
            FOLDER_OPENED,
            "Reveal in File Explorer",
            None,
            false,
            14.0,
            p,
            move || {
                let _ = std::process::Command::new("explorer.exe")
                    .arg(&win_path)
                    .spawn();
            },
        ));
    }
    pill.child(pill_btn(format!("pdel-{pid}"), TRASH, "Delete project + its sessions", Some(rgba(p.accent_red)), true, 14.0, p, {
            let tx = tx.clone();
            let pid = pid.to_string();
            let name = name.to_string();
            move || {
                let _ = tx.try_send(ShellEvent::OpenModal(crate::ui::modal::Modal {
                    title: "Delete project?".into(),
                    // Пустой проект — ОТДЕЛЬНЫЙ текст без «This cannot be
                    // undone» (`sessions-ui.ts:28`, ревью ц.21)
                    body: if count == 0 {
                        format!("Empty project “{name}” will be removed.").into()
                    } else {
                        format!(
                            "Project “{name}” and its {count} session{} will be removed. This cannot be undone.",
                            if count == 1 { "" } else { "s" }
                        )
                        .into()
                    },
                    confirm_label: "Delete".into(),
                    danger: true,
                    prompt: None,
                    placeholder: None,
                    validate: None,
                    cancel_label: None,
                    action: crate::ui::modal::ModalAction::DeleteProject(pid.clone()),
                }));
            }
        }))
        .into_any_element()
}
