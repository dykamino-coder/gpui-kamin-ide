//! Раскладка: пресеты (сохранение, применение, экспорт и импорт)
//! и пользовательские настройки приложения.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host_link::{self, ShellEvent};
use crate::state::fs_ops::{request_extensions, request_extensions_blocking};
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Раскладка: пресеты (сохранение, применение, экспорт и импорт).
    pub(crate) fn apply_layout_presets(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Cz(CzEvent::ToggleCustomize) => {
                if self.cz.legacy_bridge.is_none() {
                    self.cz.legacy_bridge = Some(crate::legacy_bridge::detect_electron_bridge());
                }
                self.cz.customize_open = !self.cz.customize_open;
                // Оригинал размонтирует CustomizeMode при выходе, а раскрытие
                // группы живёт в его `useState(true)` — при входе оно снова
                // раскрыто (ревью ц.7).
                if self.cz.customize_open {
                    self.cz.customize_contrib_collapsed.clear();
                }
                // Открытие Customize — точный момент, когда список contributed
                // страниц обязан быть свежим: снапшот реестра, снятый на
                // холодном старте, пуст, а `registry:update` может не прийти
                // вовсе (расширение уже active, а группы в наве нет).
                if self.cz.customize_open && self.cz.customize_pages.is_empty() {
                    host_link::request_status(self.tx.clone());
                }
                if self.cz.customize_open && self.cz.app_prefs.is_none() {
                    // Добор: обычно префы уже пришли по `WsConnected`, но
                    // панель могли открыть раньше ответа или после обрыва WS.
                    host_link::request_app_prefs(self.tx.clone());
                }
            }
            ShellEvent::Cz(CzEvent::SetCustomizePanel(id)) => {
                self.cz.customize_panel = id;
                if id == "extensions" && self.cz.extensions.is_none() {
                    request_extensions(self.tx.clone());
                }
            }
            ShellEvent::Cz(CzEvent::ToggleExtension(id, enabled)) => {
                // Оптимистично + RPC + перезагрузка списка (статусы обновятся)
                if let Some(exts) = self.cz.extensions.as_mut()
                    && let Some(e) = exts.iter_mut().find(|e| e.id == id)
                {
                    e.enabled = enabled;
                }
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    if let Some(c) = host_link::client() {
                        let _ = c.request(
                            "kamin:extensions:setEnabled",
                            vec![serde_json::json!(id), serde_json::json!(enabled)],
                        );
                    }
                    request_extensions_blocking(&tx);
                });
            }
            ShellEvent::Cz(CzEvent::ToggleProblemsFilter(sev)) => {
                self.problems_filter = if self.problems_filter == Some(sev) {
                    None
                } else {
                    Some(sev)
                };
            }
            ShellEvent::Cz(CzEvent::PrefsLoaded(toasts, conpty, skip_delete_confirm)) => {
                self.cz.app_prefs = Some((toasts, conpty, skip_delete_confirm));
            }
            ShellEvent::Cz(CzEvent::SetPref(key, value)) => {
                // Оптимистично + RPC в фоне
                if let Some((toasts, conpty, skip_delete_confirm)) = self.cz.app_prefs.as_mut() {
                    match key {
                        "backgroundToasts" => *toasts = value,
                        "useConptyDll" => *conpty = value,
                        "skipDeleteConfirm" => *skip_delete_confirm = value,
                        _ => {}
                    }
                }
                std::thread::spawn(move || {
                    if let Some(c) = host_link::client() {
                        let _ =
                            c.request("kamin:prefs:set", vec![serde_json::json!({ key: value })]);
                    }
                });
            }
            ShellEvent::Ed(EdEvent::TabDragOver(i)) => {
                if let Some(td) = self.ed.tab_drag.as_mut()
                    && td.started
                {
                    td.over = Some(i);
                }
            }
            ShellEvent::ToggleLayoutFlag(key) => {
                let l = &mut self.layout;
                let (field, value): (&str, bool) = match key {
                    "main" => {
                        l.main_visible = !l.main_visible;
                        ("mainVisible", l.main_visible)
                    }
                    "mainBottom" => {
                        l.main_bottom_visible = !l.main_bottom_visible;
                        ("mainBottomVisible", l.main_bottom_visible)
                    }
                    "file" => {
                        l.file_panel_visible = !l.file_panel_visible;
                        ("filePanelVisible", l.file_panel_visible)
                    }
                    "fileBottom" => {
                        l.file_panel_bottom_visible = !l.file_panel_bottom_visible;
                        ("filePanelBottomVisible", l.file_panel_bottom_visible)
                    }
                    "right" => {
                        l.right_panel_visible = !l.right_panel_visible;
                        ("rightPanelVisible", l.right_panel_visible)
                    }
                    "rightBottom" => {
                        l.right_panel_bottom_visible = !l.right_panel_bottom_visible;
                        ("rightPanelBottomVisible", l.right_panel_bottom_visible)
                    }
                    _ => return,
                };
                crate::layout_store::save_patch(serde_json::json!({ field: value }));
                // Снапшот активной сессии обновляется на КАЖДОЕ изменение
                // лейаута (см. persist_active_session_layout)
                self.persist_active_session_layout();
            }
            ShellEvent::ApplyLayoutPreset(name) => {
                let presets = crate::layout_store::load_presets();
                if let Some(pr) = presets.iter().find(|p| p.name == name) {
                    // Патч, НЕ полная замена файла: replace_layout стирал
                    // все вне-схемные ключи (открытые файлы, тему, раскладку
                    // тулов, порядок чипов) одним кликом (аудит персиста, P0).
                    crate::layout_store::save_patch(pr.snapshot.clone());
                    if let Ok(snap) =
                        serde_json::from_value::<kamin_model::LayoutSnapshot>(pr.snapshot.clone())
                    {
                        self.layout = snap;
                        self.sidebar_visible = self.layout.sidebar_visible;
                    }
                }
            }
            ShellEvent::DeleteLayoutPreset(name) => {
                let mut presets = crate::layout_store::load_presets();
                presets.retain(|p| p.name != name);
                crate::layout_store::save_presets(&presets);
            }
            ShellEvent::SetDefaultLayoutPreset(name) => {
                let mut presets = crate::layout_store::load_presets();
                for p in presets.iter_mut() {
                    p.default = p.name == name && !p.default;
                }
                crate::layout_store::save_presets(&presets);
            }
            ShellEvent::ToolDragOverTab(slot, idx) => {
                // Ховер ЧУЖОГО таба при зажатой ЛКМ = драг (порог заведомо
                // пройден при смене таба; тот же приём, что у чипов)
                let own_idx = self
                    .tool_drag
                    .as_ref()
                    .and_then(|td| self.tab_index_of(slot, &td.id));
                if let Some(td) = self.tool_drag.as_mut()
                    && (td.started || slot != td.src || Some(idx) != own_idx)
                {
                    td.started = true;
                    td.over = Some(slot);
                    td.over_index = Some(idx);
                }
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
