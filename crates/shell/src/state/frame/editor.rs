//! Перечитывание изменённых файлов редактора.
//!
//! Кусок `render` вынесен как есть (`plan/100-refactor-250.md`): порядок вызовов в кадре прежний.

use crate::file_names::{editor_lang, norm_path};
use crate::state::consts::MAX_EDITOR_TABS;
use crate::state::editor_tab::EditorTab;
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{Context, Focusable, Window};
use gpui_component::input::{InputEvent, InputState};

impl RootView {
    pub(crate) fn frame_editor_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ed.pending_reload.is_empty() {
            for p in std::mem::take(&mut self.ed.pending_reload) {
                let np = norm_path(&p);
                let Some(idx) = self
                    .ed
                    .editor_tabs
                    .iter()
                    .position(|t| !t.dirty && norm_path(&t.path) == np)
                else {
                    continue;
                };
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                let tab_path = self.ed.editor_tabs[idx].path.clone();
                let input = self.ed.editor_tabs[idx].input.clone();
                self.ed.reload_suppress.insert(tab_path);
                input.update(cx, |st, cx| st.set_value(text, window, cx));
            }
        }
    }

    pub(crate) fn frame_editor_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ed.pending_editor.is_none()
            && let Some((path, line)) = self.ed.pending_goto.take()
            && let Some(tab) = self.ed.editor_tabs.iter().find(|t| t.path == path)
        {
            let input = tab.input.clone();
            input.update(cx, |st, cx| {
                st.set_cursor_position(
                    gpui_component::input::Position::new(line.saturating_sub(1), 0),
                    window,
                    cx,
                );
            });
        }
        if let Some((path, text)) = self.ed.pending_editor.take() {
            let lang = editor_lang(&path);
            let eol = if text.contains("\r\n") { "CRLF" } else { "LF" };
            // LSP поверх редактора: hover/definition через exthost-каналы
            // хоста + doc-sync (те же провайдеры расширений, что у Monaco)
            let lsp = std::rc::Rc::new(crate::editor_lsp::HostLsp::new(&path, lang));
            lsp.open(&text);
            let mirror_src = text.clone();
            let input = cx.new(|cx| {
                let mut st = InputState::new(window, cx)
                    .code_editor(lang)
                    .soft_wrap(false);
                st.lsp.hover_provider = Some(lsp.clone());
                st.lsp.definition_provider = Some(lsp.clone());
                st.set_value(text, window, cx);
                st
            });
            // Зеркало для минимапы: тот же текст и язык, свой layout —
            // это и есть `minimap_editor` из Zed, только на нашем Input.
            let mirror_text = self
                .ed
                .editor_tabs
                .iter()
                .find(|t| t.path == path)
                .map(|_| String::new());
            let _ = mirror_text;
            self.ed.minimap_input = Some(cx.new(|cx| {
                // Zed-минимапа не рисует номера строк и не подсвечивает
                // текущую строку/выделение — это чистый силуэт текста.
                let mut st = InputState::new(window, cx)
                    .code_editor(lang)
                    .line_number(false)
                    // Zed `EditorMode::Minimap`: read-only, без подписок и
                    // каретки. У нас этот флаг ещё и снимает жёсткий
                    // line-height `Input`-а, иначе строки идут через 20px.
                    .minimap()
                    .soft_wrap(false);
                st.set_value(mirror_src.clone(), window, cx);
                st
            }));
            // Change → dirty ЭТОГО таба (ищем по path — индексы плавают)
            let sub_path = path.clone();
            let sub = cx.subscribe(&input, move |this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    // Программный set_value при внешнем reload — не dirty
                    if this.ed.reload_suppress.remove(&sub_path) {
                        return;
                    }
                    if let Some(tab) = this.ed.editor_tabs.iter_mut().find(|t| t.path == sub_path) {
                        tab.dirty = true;
                        this.ed.minimap_stale = true;
                        cx.notify();
                    }
                }
            });
            window.focus(&input.read(cx).focus_handle(cx));
            // LRU-лимит: 13-й таб вытесняет самый давний ЧИСТЫЙ (dirty не трогаем)
            if self.ed.editor_tabs.len() >= MAX_EDITOR_TABS
                && let Some(evict) = self
                    .ed
                    .editor_tabs
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| !t.dirty && !t.pinned)
                    .min_by_key(|(_, t)| t.last_used)
                    .map(|(i, _)| i)
            {
                crate::editor_lsp::HostLsp::close(self.ed.editor_tabs[evict].path.clone());
                self.ed.editor_tabs.remove(evict);
                if self.ed.editor_active > evict {
                    self.ed.editor_active -= 1;
                }
            }
            self.ed.editor_tabs.push(EditorTab {
                path,
                input,
                dirty: false,
                eol,
                last_used: std::time::Instant::now(),
                pinned: false,
                _sub: sub,
            });
            self.persist_open_files();
            self.ed.editor_active = self.ed.editor_tabs.len() - 1;
            self.ed.tabs_reveal_active = true;
            if self.ed.pending_goto.is_some() {
                cx.notify(); // ещё кадр — goto применится после layout инпута
            }
        }
        // LRU-штамп активного таба (каждый кадр — всегда актуален)
        if let Some(tab) = self.ed.editor_tabs.get_mut(self.ed.editor_active) {
            tab.last_used = std::time::Instant::now();
        }
        // Выделение в дереве следует за активным файлом. Оригинал — ЭФФЕКТ на
        // смену `selectedFile` (`file-selection.ts:57-63`), а не работа каждый
        // кадр: покадровый синк затирал Ctrl-мультивыделение и выделение папки
        // в тот же кадр, в котором они появлялись (ревью ц.13).
    }
}
