//! Контекст кадра: что посчитано ДО сборки дерева и нужно ниже по коду.
//!
//! Пролог `render` перенесён как есть (`plan/100-refactor-250.md`).

use crate::state::model::RootView;
use gpui::{Context, Window};
use kamin_theme::Palette;

/// Значения кадра, от которых зависит сборка колонок.
pub(crate) struct FrameCtx {
    pub p: &'static Palette,
    pub log_filter_focused: bool,
    pub design_input_focused: bool,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub sidebar_w: f32,
    pub file_w: f32,
    pub right_w: f32,
    pub main_split: f32,
    pub file_bottom_px: f32,
    pub has_active: bool,
    pub no_sessions: bool,
}

impl RootView {
    pub(crate) fn frame_ctx(&mut self, window: &mut Window, cx: &mut Context<Self>) -> FrameCtx {
        let p = self.theme.palette();
        // `:focus-within` у поля поиска логов — панели окна не видят
        let log_filter_focused =
            self.cz.log_filter_input.as_ref().is_some_and(|inp| {
                gpui::Focusable::focus_handle(inp.read(cx), cx).is_focused(window)
            });
        // `.input:focus { border-color: accent-primary }` семпла Design
        let design_input_focused =
            self.cz.design_input.as_ref().is_some_and(|inp| {
                gpui::Focusable::focus_handle(inp.read(cx), cx).is_focused(window)
            });
        self.frame_focus(window, cx);
        self.frame_overlay(window, cx);
        self.frame_webviews(window, cx);
        // Редактор: создать таб (InputState::code_editor) для ждущего файла
        // Scroll-to-line: ПЕРЕД pending_editor — для нового файла goto
        // остаётся на следующий кадр (инпут должен пройти layout, иначе
        // move_to не проскроллит вьюпорт).
        self.frame_editor_open(window, cx);
        let active_path = self
            .ed
            .editor_tabs
            .get(self.ed.editor_active)
            .map(|t| t.path.clone());
        if active_path != self.tree_synced_path {
            self.tree_synced_path = active_path.clone();
            if let Some(path) = active_path {
                self.select_tree_path(&path, cx);
            }
        }
        // Внешние изменения (watcher): reload чистых табов новым содержимым
        self.frame_editor_reload(window, cx);
        // Видимость wv2-child'ов: невидимые в layout — set_visible(false),
        self.frame_panels(window, cx);
        // Inline-rename: лениво создать фокусируемый инпут (seed = имя сессии)
        self.frame_inputs(window, cx);
        let crate::state::metrics::Metrics {
            viewport_w,
            viewport_h,
            sidebar_w,
            file_w,
            right_w,
            main_split,
            file_bottom_px,
        } = self.frame_metrics(window);
        // остальные панели живут как обычно — 1:1 ContributedContainerBody
        let has_active = self
            .sessions
            .as_ref()
            .and_then(|s| s.active_session_id.as_deref())
            .is_some();
        // Нет НИ ОДНОЙ открытой сессии → показываем только сайдбар и welcome:
        // File/Right/Main-Bottom несут исключительно сессионное содержимое
        // (файлы, консоль, todos), и без сессии они пустые. Тумблеры видимости
        // не меняем — раскладка вернётся при открытии сессии
        // (`AppLayout.tsx:51,69,72,73`).
        let no_sessions = !self.cz.customize_open
            && self
                .sessions
                .as_ref()
                .is_none_or(|s| !s.sessions.iter().any(|x| x.open));
        // Файл-табы привязаны к сессии (как веб-страницы в Web): на смене
        // активной — стэш текущего набора и подъём набора новой сессии.
        // Первая привязка (None → Some) НЕ стэшит: табы, открытые до первого
        // снапшота, достаются первой активной сессии.
        {
            let active_sid: Option<String> = self
                .sessions
                .as_ref()
                .and_then(|s| s.active_session_id.clone());
            if active_sid != self.ed.editor_tabs_session {
                if let Some(old) = self.ed.editor_tabs_session.take() {
                    let tabs = std::mem::take(&mut self.ed.editor_tabs);
                    // Веб-страница уходит в стэш вместе с табами
                    if let Some(inp) = &self.browser_input {
                        let url = inp.read(cx).value().to_string();
                        if !url.is_empty() {
                            self.browser_url_stash.insert(old.clone(), url);
                        }
                    }
                    self.ed
                        .editor_tabs_stash
                        .insert(old, (tabs, self.ed.editor_active));
                    self.ed.editor_active = 0;
                }
                // Подъём набора новой сессии; бут-табы (old=None, непустые)
                // не выбрасываем — они и достаются новой сессии
                if let Some(sid) = &active_sid
                    && self.ed.editor_tabs.is_empty()
                    && let Some((tabs, act)) = self.ed.editor_tabs_stash.remove(sid)
                {
                    self.ed.editor_tabs = tabs;
                    self.ed.editor_active = act;
                }
                // Веб новой сессии: восстановить её URL (страницы привязаны
                // к сессии); нет записи → страница остаётся как была
                if let Some(sid) = &active_sid
                    && let Some(url) = self.browser_url_stash.get(sid).cloned()
                    && let Some(inp) = &self.browser_input
                {
                    let differs = inp.read(cx).value() != url.as_str();
                    if differs {
                        inp.update(cx, |st, cx| st.set_value(url.clone(), window, cx));
                        crate::web::navigate("browser", &url);
                    }
                }
                self.ed.editor_tabs_session = active_sid;
            }
        }
        FrameCtx {
            p,
            log_filter_focused,
            design_input_focused,
            viewport_w,
            viewport_h,
            sidebar_w,
            file_w,
            right_w,
            main_split,
            file_bottom_px,
            has_active,
            no_sessions,
        }
    }
}
