//! Раскладка: активити-бар, закрепление инструментов в слотах,
//! тумблеры панелей и перетаскивание табов.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::state::fs_ops::request_extensions;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Раскладка: активити-бар, закрепление инструментов в слотах.
    pub(crate) fn apply_layout_panels(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Cz(CzEvent::ToggleCustomizeContribGroup(id)) => {
                if !self.cz.customize_contrib_collapsed.remove(&id) {
                    self.cz.customize_contrib_collapsed.insert(id);
                }
            }
            ShellEvent::ToggleTabsOverflow(x, y) => {
                self.close_popovers_except("tabs");
                // (0,0) = принудительное закрытие (клик по пункту)
                self.tabs_overflow_open = if self.tabs_overflow_open.is_some() {
                    None
                } else {
                    Some((x, y))
                };
            }
            ShellEvent::ActivityClicked(id) => {
                // Клик по тулу уводит из настроек (оригинал)
                self.cz.customize_open = false;
                self.sidebar_activity = id;
                // ...и переключает ТЕЛО сайдбара: у оригинала
                // `Sidebar.tsx:81-85` рендерит `ActivityBody` активного тула
                self.activity
                    .set_active(crate::activity::PanelSlot::Sidebar, id);
                // Списку расширений нужны данные — тянем при активации тула
                // (в Customize это делает открытие страницы Extensions)
                if id == "extensions" && self.cz.extensions.is_none() {
                    request_extensions(self.tx.clone());
                }
            }
            ShellEvent::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
                crate::layout_store::save_patch(
                    serde_json::json!({"sidebarVisible": self.sidebar_visible}),
                );
                self.persist_active_session_layout();
            }
            ShellEvent::ToggleInactive(pid) => {
                if !self.inactive_open.remove(&pid) {
                    self.inactive_open.insert(pid);
                }
            }
            ShellEvent::OverwriteLayoutPreset(name) => {
                let mut presets = crate::layout_store::load_presets();
                if let Some(pr) = presets.iter_mut().find(|p| p.name == name)
                    && let Ok(v) = serde_json::to_value(&self.layout)
                {
                    pr.snapshot = v;
                    crate::layout_store::save_presets(&presets);
                }
            }
            ShellEvent::ExportPreset(name) => {
                let presets = crate::layout_store::load_presets();
                let Some(pr) = presets.into_iter().find(|p| p.name == name) else {
                    return;
                };
                let file = name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '.' || c == '-' {
                            c
                        } else {
                            '-'
                        }
                    })
                    .collect::<String>();
                let rx = cx.prompt_for_new_path(
                    std::path::Path::new(&std::env::var("USERPROFILE").unwrap_or_default()),
                    Some(&format!("{file}.layout.json")),
                );
                let tx = self.tx.clone();
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(path))) = rx.await {
                        let body = serde_json::json!({
                            "kaminLayout": 1,
                            "name": pr.name,
                            "snapshot": pr.snapshot,
                        });
                        let ok = serde_json::to_string_pretty(&body)
                            .ok()
                            .and_then(|b| std::fs::write(&path, b).ok())
                            .is_some();
                        let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                            id: "preset-export-one".into(),
                            severity: if ok { "success" } else { "error" }.into(),
                            title: None,
                            message: if ok {
                                "Layout exported".into()
                            } else {
                                "Export failed".into()
                            },
                            actions: Vec::new(),
                            sticky: false,
                        }));
                    }
                })
                .detach();
            }
            ShellEvent::ExportPresets => {
                let rx = cx.prompt_for_new_path(
                    std::path::Path::new(&std::env::var("USERPROFILE").unwrap_or_default()),
                    Some("kamin-layout.json"),
                );
                // Оригинал: «Export current layout…» пишет ТЕКУЩИЙ layout
                // одним файлом {kaminLayout, name, snapshot}
                let body = serde_json::json!({
                    "kaminLayout": 1,
                    "name": "Layout",
                    "snapshot": serde_json::to_value(&self.layout).unwrap_or_default(),
                });
                let tx = self.tx.clone();
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(path))) = rx.await {
                        let ok = serde_json::to_string_pretty(&body)
                            .ok()
                            .and_then(|body| std::fs::write(&path, body).ok())
                            .is_some();
                        let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                            id: "preset-export".into(),
                            severity: if ok { "success" } else { "error" }.into(),
                            title: None,
                            message: if ok {
                                "Layouts exported".into()
                            } else {
                                "Export failed".into()
                            },
                            actions: Vec::new(),
                            sticky: false,
                        }));
                    }
                })
                .detach();
            }
            ShellEvent::ImportPresets => {
                let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                let tx = self.tx.clone();
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(paths))) = rx.await
                        && let Some(p) = paths.first()
                    {
                        // Массив пресетов (наш экспорт) ИЛИ одиночный
                        // файл оригинала {kaminLayout, name, snapshot}
                        let imported: Vec<crate::layout_store::LayoutPreset> =
                            std::fs::read_to_string(p)
                                .ok()
                                .and_then(|s| {
                                    serde_json::from_str::<Vec<crate::layout_store::LayoutPreset>>(
                                        &s,
                                    )
                                    .ok()
                                    .or_else(|| {
                                        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
                                        let snapshot = v.get("snapshot")?.clone();
                                        let name = v
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("Imported")
                                            .to_string();
                                        Some(vec![crate::layout_store::LayoutPreset {
                                            name,
                                            snapshot,
                                            default: false,
                                        }])
                                    })
                                })
                                .unwrap_or_default();
                        let n = imported.len();
                        if n > 0 {
                            // Merge по имени: импортированные перекрывают
                            let mut cur = crate::layout_store::load_presets();
                            for im in imported {
                                cur.retain(|c| c.name != im.name);
                                cur.push(im);
                            }
                            crate::layout_store::save_presets(&cur);
                        }
                        let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                            id: "preset-import".into(),
                            severity: if n > 0 { "success" } else { "warning" }.into(),
                            title: None,
                            message: if n > 0 {
                                format!("Imported {n} layout(s)")
                            } else {
                                "No layouts found in file".into()
                            },
                            actions: Vec::new(),
                            sticky: false,
                        }));
                    }
                })
                .detach();
            }
            ShellEvent::PinTool(slot, id) => {
                self.activity.pin(slot, &id);
                self.save_activity();
                self.tool_picker = None;
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
