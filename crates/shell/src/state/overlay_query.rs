//! Гейт палитры и активные индексы оверлеев поиска.
//!
//! Методы перенесены из `root.rs` дословно (`plan/100-refactor-250.md`).

use crate::host_link::{self};
use crate::state::model::RootView;
impl RootView {
    /// Гейт палитры на ТЕКУЩИХ ключах контекста: реестровые ключи плюс
    /// локальные, которые оригинал держит в `localContextKeys`
    /// (`MonacoEditor.tsx:216-294`) — язык и расширение активного файла.
    pub fn palette_gate(&self) -> std::collections::HashMap<String, bool> {
        let mut ctx = self.context_keys.clone();
        if let Some(tab) = self.ed.editor_tabs.get(self.ed.editor_active) {
            let name = tab.path.rsplit(['/', '\\']).next().unwrap_or("");
            let ext = name
                .rfind('.')
                .filter(|i| *i > 0)
                .map(|i| name[i..].to_string())
                .unwrap_or_default();
            ctx.insert("resourceExtname".into(), serde_json::json!(ext));
            ctx.insert("resourceFilename".into(), serde_json::json!(name));
            ctx.insert("resourceScheme".into(), serde_json::json!("file"));
            ctx.insert("editorFocus".into(), serde_json::json!(true));
        }
        crate::ui::command_palette::palette_gate(&self.palette_menu, &ctx)
    }

    /// Запись в system-лог (кап 500 записей).
    pub(crate) fn push_syslog(&mut self, level: &'static str, source: &str, message: &str) {
        self.cz.system_log.push(crate::output_log::SysEntry {
            level,
            source: source.to_string(),
            message: message.to_string(),
            at: std::time::SystemTime::now(),
        });
        if self.cz.system_log.len() > 500 {
            let drop = self.cz.system_log.len() - 500;
            self.cz.system_log.drain(..drop);
        }
    }

    /// Сброс состояния Go to Symbol.
    pub(crate) fn close_ws(&mut self) {
        self.sov.ws_input = None;
        self.sov.ws_sub = None;
        self.sov.ws_results.clear();
        self.sov.ws_query_len = 0;
    }

    /// Исполнить подтверждённое действие модалки. `input` — значение
    /// prompt-инпута (CreateEntry/RenameFs). Host-RPC/fs — в фоне.
    /// Индексы активных строк оверлеев с кламом по длине списка.
    pub fn qo_active_idx(&self) -> usize {
        self.sov
            .qo_active
            .min(self.sov.quickopen_results.len().saturating_sub(1))
    }

    pub fn fif_active_idx(&self) -> usize {
        self.sov
            .fif_active
            .min(self.sov.fif_results.len().saturating_sub(1))
    }

    pub fn ws_active_idx(&self) -> usize {
        self.sov
            .ws_active
            .min(self.sov.ws_results.len().saturating_sub(1))
    }

    /// ↑/↓ в открытом инпут-оверлее: сдвиг активной строки с кламом
    /// (`Math.min(len-1, a+1)` / `Math.max(0, a-1)` оригинала).
    pub(crate) fn move_overlay_active(&mut self, delta: i32) {
        let (active, len) = if self.sov.ws_open {
            (&mut self.sov.ws_active, self.sov.ws_results.len())
        } else if self.sov.fif_open {
            (&mut self.sov.fif_active, self.sov.fif_results.len())
        } else if self.sov.quickopen_open {
            (&mut self.sov.qo_active, self.sov.quickopen_results.len())
        } else {
            return;
        };
        if len == 0 {
            *active = 0;
            return;
        }
        let next = (*active as i32 + delta).clamp(0, len as i32 - 1);
        *active = next as usize;
    }

    /// Раскрыть все директории, чей листинг уже загружен, и запросить
    /// недостающие (каскад Expand-All оригинала). Кап 2000 директорий —
    /// защита от разворачивания гигантского дерева одним кликом.
    pub(crate) fn expand_loaded_dirs(&mut self, cx: &mut gpui::App) {
        const CASCADE_CAP: usize = 2000;
        let dirs: Vec<String> = self
            .tree(cx)
            .cache
            .iter()
            .flat_map(|(dir, entries)| {
                entries
                    .iter()
                    .filter(|e| e.is_dir)
                    .map(move |e| crate::ui::file_list::join(dir, &e.name))
            })
            .take(CASCADE_CAP)
            .collect();
        // ОДНОЙ мутацией на весь каскад: `tree_mut` на каждую из 2000
        // директорий давал 2000 уведомлений панели на один клик (ревью).
        let requests = self.tree_mut(cx, |tree| {
            let mut requests = Vec::new();
            for path in dirs {
                if tree.expanded.insert(path.clone())
                    && !tree.cache.contains_key(&path)
                    && tree.loading.insert(path.clone())
                {
                    requests.push(path);
                }
            }
            requests
        });
        for path in requests {
            host_link::request_list_dir(self.tx.clone(), path);
        }
    }
}
