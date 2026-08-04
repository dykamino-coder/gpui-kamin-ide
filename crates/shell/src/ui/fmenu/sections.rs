//! Секции файлового меню: встроенные пункты и вклады расширений.
//!
//! Тела секций перенесены из `file_menu` дословно (`plan/100-refactor-250.md`).

use crate::host::events::EdEvent;
use crate::host::events::ShellEvent;
pub(crate) use crate::ui::fmenu::contrib::contributed_items;
use crate::ui::fmenu::items::{
    FA_COPY, FA_CUT, FA_LINK, FA_NEW_FILE, FA_NEW_FOLDER, FA_PASTE, FA_PEN, FA_ROUTE, FA_TRASH,
    divider, item, item_owned,
};
use crate::ui::fmenu::model::base_name;
use crate::ui::fmenu::model::{FileMenu, name_error};
use crate::ui::modal::{Modal, ModalAction};
use gpui::SharedString;
use gpui::prelude::*;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Встроенные пункты: New, буфер, переименование, удаление, пути.
#[allow(clippy::too_many_arguments)]
pub(crate) fn builtin_items(
    mut rest: gpui::Stateful<gpui::Div>,
    p: &Palette,
    tx: &Sender<ShellEvent>,
    path: &str,
    is_dir: bool,
    paste_dir: String,
    menu: &FileMenu,
) -> gpui::Stateful<gpui::Div> {
    // ── New (только папка)
    if is_dir {
        let dir_nf = path.to_string();
        let dir_nd = path.to_string();
        rest = rest
            .child(item("fm-newfile", FA_NEW_FILE, "New File…", false, p, {
                let tx = tx.clone();
                move || {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                    let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                        title: "New File".into(),
                        body: "".into(),
                        // `showPrompt` оригинала: кнопка всегда «OK»,
                        // плейсхолдер «name», валидатор — `nameError`
                        confirm_label: "OK".into(),
                        danger: false,
                        prompt: Some(String::new()),
                        placeholder: Some("name".into()),
                        validate: Some(name_error),
                        cancel_label: None,
                        action: ModalAction::CreateEntry {
                            dir: dir_nf.clone(),
                            folder: false,
                        },
                    }));
                }
            }))
            .child(item(
                "fm-newfolder",
                FA_NEW_FOLDER,
                "New Folder…",
                false,
                p,
                {
                    let tx = tx.clone();
                    move || {
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                        let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                            title: "New Folder".into(),
                            body: "".into(),
                            confirm_label: "OK".into(),
                            danger: false,
                            prompt: Some(String::new()),
                            placeholder: Some("name".into()),
                            validate: Some(name_error),
                            cancel_label: None,
                            action: ModalAction::CreateEntry {
                                dir: dir_nd.clone(),
                                folder: true,
                            },
                        }));
                    }
                },
            ))
            .child(divider(p));
    }

    // ── Clipboard (мультиселект → операция над всем выбором)
    let clip_paths: Vec<String> = if menu.multi.len() > 1 {
        menu.multi.clone()
    } else {
        vec![path.to_string()]
    };
    rest = rest
        .child(item("fm-cut", FA_CUT, "Cut", false, p, {
            let tx = tx.clone();
            let paths = clip_paths.clone();
            move || {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsCut(paths.clone())));
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
            }
        }))
        .child(item("fm-copy", FA_COPY, "Copy", false, p, {
            let tx = tx.clone();
            let paths = clip_paths.clone();
            move || {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsCopy(paths.clone())));
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
            }
        }))
        .child(item("fm-paste", FA_PASTE, "Paste", false, p, {
            let tx = tx.clone();
            move || {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FsPaste(paste_dir.clone())));
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
            }
        }))
        .child(divider(p));

    // ── Modify
    let name = base_name(path);
    rest = rest
            .child(item("fm-rename", FA_PEN, "Rename…", false, p, {
                let tx = tx.clone();
                let path = path.to_string();
                let name = name.clone();
                move || {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                    let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                        title: "Rename".into(),
                        body: "".into(),
                        confirm_label: "OK".into(),
                        danger: false,
                        prompt: Some(name.clone()),
                        placeholder: None,
                        validate: Some(name_error),
                        cancel_label: None,
                        action: ModalAction::RenameFs { path: path.clone() },
                    }));
                }
            }))
            .child({
                let multi = menu.multi.clone();
                if multi.len() > 1 {
                    let n = multi.len();
                    let tx = tx.clone();
                    item_owned(
                        "fm-delete",
                        FA_TRASH,
                        format!("Delete {n} items"),
                        true,
                        p,
                        move || {
                            let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                            let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                                title: "Delete".into(),
                                body: SharedString::from(format!(
                                    "{n} items will be moved to the Recycle Bin (Ctrl+Z to undo)."
                                )),
                                confirm_label: "Delete".into(),
                                danger: true,
                                prompt: None,
                                placeholder: None,
                                validate: None,
                                cancel_label: None,
                                action: ModalAction::DeleteFsMany {
                                    paths: multi.clone(),
                                },
                            }));
                        },
                    )
                } else {
                    let tx = tx.clone();
                    let path = path.to_string();
                    let name = name.clone();
                    // Зарезервированное имя устройства в корзину не уходит —
                    // оригинал предлагает удалить навсегда
                    // (`file-context-menu.ts::deleteEntry`)
                    let reserved = crate::file_names::is_reserved_name(&name);
                    item("fm-delete", FA_TRASH, "Delete", true, p, move || {
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                        let _ = tx.try_send(ShellEvent::OpenModal(Modal {
                            title: "Delete".into(),
                            body: SharedString::from(if reserved {
                                format!(
                                    "<b>{name}</b> is a reserved system name and can't go to the Recycle Bin. Delete it permanently?"
                                )
                            } else {
                                format!("Move <b>{name}</b> to the Recycle Bin?")
                            }),
                            confirm_label: if reserved {
                                "Delete permanently".into()
                            } else {
                                "Move to Recycle Bin".into()
                            },
                            danger: true,
                            prompt: None,
                            placeholder: None,
                            validate: None,
                            cancel_label: None,
                            action: ModalAction::DeleteFs {
                                path: path.clone(),
                                is_dir,
                            },
                        }));
                    })
                }
            })
            .child(divider(p));

    // ── Copy path
    rest = rest
        .child(item("fm-copypath", FA_LINK, "Copy Path", false, p, {
            let tx = tx.clone();
            let path = path.to_string();
            move || {
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CopyToClipboard(path.clone())));
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
            }
        }))
        .child(item(
            "fm-copyrel",
            FA_ROUTE,
            "Copy Relative Path",
            false,
            p,
            {
                let tx = tx.clone();
                let path = path.to_string();
                move || {
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CopyRelativePath(path.clone())));
                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseFileMenu));
                }
            },
        ));
    rest
}
