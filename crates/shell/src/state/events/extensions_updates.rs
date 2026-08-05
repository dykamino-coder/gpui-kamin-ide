//! Расширения и самообновление: список и статусы расширений,
//! удаление, проверка и установка новой версии IDE.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host_link::{self, ShellEvent};
use crate::state::fs_ops::request_extensions_blocking;
use gpui::Context;

use crate::root::{RootView, system_theme};

impl RootView {
    /// Расширения и самообновление: список и статусы расширений.
    pub(crate) fn apply_extensions_updates(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Cz(CzEvent::ExtensionsLoaded(list)) => {
                // Иконки тянем по одной на id, как `iconCache` оригинала
                // (`ExtensionsPanel.tsx:14-26`): повторно не запрашиваем
                for e in &list {
                    if !self.cz.ext_icons.contains_key(&e.id) {
                        // Заглушка «запрошено», чтобы не слать дубли
                        self.cz.ext_icons.insert(e.id.clone(), None);
                        host_link::request_extension_icon(self.tx.clone(), e.id.clone());
                    }
                }
                self.cz.extensions = Some(list);
            }
            ShellEvent::Cz(CzEvent::ExtensionsStatus(s)) => self.cz.ext_status = s,
            ShellEvent::Cz(CzEvent::UninstallExtension(id)) => {
                // Оптимистично убрать строку + RPC + перезагрузка списка
                if let Some(exts) = self.cz.extensions.as_mut() {
                    exts.retain(|e| e.id != id);
                }
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    if let Some(c) = host_link::client() {
                        let _ =
                            c.request("kamin:extensions:uninstall", vec![serde_json::json!(id)]);
                    }
                    request_extensions_blocking(&tx);
                });
            }
            ShellEvent::Commands(cmds) => self.commands = cmds,
            ShellEvent::Cz(CzEvent::StatusCounts(counts)) => self.status_counts = counts,
            ShellEvent::Cz(CzEvent::StartUpdateInstall) => {
                if self.update_progress.is_none()
                    && let Some((_, url)) = self.update_available.clone()
                {
                    self.update_progress = Some((0, None));
                    crate::updater::install(url, self.tx.clone());
                }
            }
            ShellEvent::Cz(CzEvent::UpdateProgress(done, total)) => {
                self.update_progress = Some((done, total));
            }
            ShellEvent::Cz(CzEvent::GracefulQuit) => {
                // apply без окна — закрытие сделает frame_focus ближайшего кадра.
                self.pending_quit = true;
                cx.notify();
            }
            ShellEvent::Cz(CzEvent::UpdateInstallFailed(err)) => {
                self.update_progress = None;
                let _ = self
                    .tx
                    .try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                        id: "update-failed".into(),
                        severity: "error".into(),
                        title: None,
                        message: format!("Update failed: {err}"),
                        actions: vec!["Dismiss".into()],
                        sticky: true,
                    }));
            }
            ShellEvent::SetThemeChoice(choice) => {
                // Builtin Dark/Light/System сбрасывают contributed-тему
                // (включая её бут-кэш — иначе следующий старт поднял бы её)
                kamin_theme::set_contributed(None);
                crate::ui::webview_theme::set_contrib_colors(None);
                crate::theme_sync::set_contrib_syntax(None);
                let _ = std::fs::remove_file(crate::theme_sync::theme_cache_path());
                self.contrib_theme_id = None;
                self.theme_choice = choice;
                self.theme = match choice {
                    "light" => kamin_theme::ThemeKind::Light,
                    "system" => system_theme(cx),
                    _ => kamin_theme::ThemeKind::Dark,
                };
                crate::theme_sync::apply(self.theme, cx);
                self.rebake_welcome_glow();
                // Живым вебвью тему доносит postMessage — без перезагрузки
                crate::ui::webview_theme::push_live();
                crate::layout_store::save_patch(serde_json::json!({
                    "themeChoice": choice,
                    "contributedThemeId": serde_json::Value::Null,
                }));
            }
            ShellEvent::Cz(CzEvent::SetContributedTheme(id, path, dark_ui)) => {
                let base = if dark_ui {
                    kamin_theme::ThemeKind::Dark
                } else {
                    kamin_theme::ThemeKind::Light
                };
                if crate::theme_sync::apply_contributed(&path, dark_ui, cx) {
                    self.theme = base;
                    self.rebake_welcome_glow();
                    crate::ui::webview_theme::push_live();
                    // Persist как kamin.contributedThemeId оригинала; dark_ui
                    // нужен буту из кэша (kind до чтения самой темы)
                    crate::layout_store::save_patch(serde_json::json!({
                        "contributedThemeId": &id,
                        "contributedThemeDarkUi": dark_ui,
                    }));
                    self.contrib_theme_id = Some(id);
                } else {
                    self.push_syslog("error", "theme", &format!("failed to load {path}"));
                }
            }
            ShellEvent::RunCommand(id) => {
                self.palette_open = false;
                self.palette_input = None;
                self.palette_sub = None;
                std::thread::spawn(move || {
                    if let Some(client) = host_link::client() {
                        let _ =
                            client.request("kamin:command:execute", vec![serde_json::json!(id)]);
                    }
                });
            }
            ShellEvent::Cz(CzEvent::CheckForUpdates) => {
                // Как updater_check у Tauri-оригинала: база — Bridge server из
                // конфига хоста (kamin:bridge:serverUrl), эндпоинт Tauri-протокола
                // /updates/kaminide/{target}/{arch}/{current}. Хостового метода
                // kamin:updater:check НЕ существует (тост-жалоба юзера).
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let toast = |sev: &str, msg: String| crate::ui::toasts::Toast {
                        id: "updater-check".into(),
                        severity: sev.into(),
                        title: None,
                        message: msg,
                        actions: Vec::new(),
                        sticky: false,
                    };
                    let base = crate::host_link::client()
                        .and_then(|c| c.request("kamin:bridge:serverUrl", vec![]).ok())
                        .and_then(|v| v.as_str().map(|s| s.trim_end_matches('/').to_string()))
                        // Конфиг хранит WS-адрес сервера (ws://host:3456) —
                        // updater ходит тем же хостом по HTTP (жалоба юзера:
                        // «bad uri: unknown scheme: ws»).
                        .map(|s| {
                            if let Some(r) = s.strip_prefix("ws://") {
                                format!("http://{r}")
                            } else if let Some(r) = s.strip_prefix("wss://") {
                                format!("https://{r}")
                            } else {
                                s
                            }
                        });
                    let Some(base) = base else {
                        let _ = tx.try_send(ShellEvent::Toast(toast(
                            "warning",
                            "Update check failed: Bridge server URL is not configured".into(),
                        )));
                        return;
                    };
                    let url = format!(
                        "{base}/updates/kaminide/windows/x86_64/{}",
                        env!("CARGO_PKG_VERSION")
                    );
                    match ureq::get(&url).call() {
                        Ok(mut resp) => {
                            // 204/битый JSON = новее нет (протокол Tauri).
                            let text = resp.body_mut().read_to_string().unwrap_or_default();
                            let newer = serde_json::from_str::<serde_json::Value>(&text)
                                .ok()
                                .and_then(|b| {
                                    crate::updater::parse_latest(&b, env!("CARGO_PKG_VERSION"))
                                });
                            match newer {
                                Some(info) => {
                                    let _ = tx.try_send(ShellEvent::Toast(toast(
                                        "info",
                                        format!("Update available: KaminIDE {}", info.version),
                                    )));
                                    let _ = tx.try_send(ShellEvent::Cz(CzEvent::UpdateAvailable(
                                        info.version,
                                        info.url,
                                    )));
                                }
                                None => {
                                    let _ = tx.try_send(ShellEvent::Toast(toast(
                                        "info",
                                        "You are up to date".into(),
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.try_send(ShellEvent::Toast(toast(
                                "warning",
                                format!("Update check failed ({url}): {e}"),
                            )));
                        }
                    }
                });
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
