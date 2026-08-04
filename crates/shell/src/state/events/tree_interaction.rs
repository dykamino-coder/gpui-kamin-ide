//! Дерево файлов: клик и reveal, перетаскивание узлов, чекбоксы
//! contributed-деревьев, безвозвратное удаление.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Дерево файлов: клик и reveal, перетаскивание узлов, чекбоксы.
    pub(crate) fn apply_tree_interaction(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Tree(TreeEvent::TreeClick {
                view,
                handle,
                expandable,
                expanded,
                command,
            }) => {
                let st = self.trees.entry(view.clone()).or_default();
                if expandable {
                    st.expanded.insert(handle.clone(), expanded);
                    host_link::report_tree(
                        "kamin:tree:reportExpansion",
                        vec![
                            serde_json::json!(view),
                            serde_json::json!(handle),
                            serde_json::json!(expanded),
                        ],
                    );
                    if expanded && !st.levels.contains_key(&handle) {
                        st.levels.insert(handle.clone(), None);
                        host_link::request_tree_children(
                            self.tx.clone(),
                            view.clone(),
                            handle.clone(),
                        );
                    }
                }
                st.selected = Some(handle.clone());
                host_link::report_tree(
                    "kamin:tree:reportSelection",
                    vec![serde_json::json!(view), serde_json::json!([handle])],
                );
                // Клик выполняет команду узла и на листьях, и на родителях
                if let Some((name, args)) = command {
                    host_link::report_tree(
                        "kamin:command:execute",
                        std::iter::once(serde_json::json!(name))
                            .chain(args)
                            .collect(),
                    );
                }
            }
            ShellEvent::Tree(TreeEvent::TreeReveal {
                view,
                handle,
                expand_path,
                select,
                expand,
                tries,
            }) => {
                let tx = self.tx.clone();
                let st = self.trees.entry(view.clone()).or_default();
                // Цепочку предков раскрываем принудительно, иначе целевой
                // узел просто не отрисуется (`tree-views.ts:52`)
                let mut pending = false;
                for h in &expand_path {
                    st.expanded.insert(h.clone(), true);
                    if !st.levels.contains_key(h) {
                        st.levels.insert(h.clone(), None);
                        host_link::request_tree_children(tx.clone(), view.clone(), h.clone());
                    }
                    if st.levels.get(h).is_none_or(Option::is_none) {
                        pending = true;
                    }
                }
                if select {
                    st.selected = Some(handle.clone());
                }
                if expand && !st.expanded.get(&handle).copied().unwrap_or(false) {
                    st.expanded.insert(handle.clone(), true);
                    if !st.levels.contains_key(&handle) {
                        st.levels.insert(handle.clone(), None);
                        host_link::request_tree_children(tx.clone(), view.clone(), handle.clone());
                    }
                    host_link::report_tree(
                        "kamin:tree:reportExpansion",
                        vec![
                            serde_json::json!(view),
                            serde_json::json!(handle),
                            serde_json::json!(true),
                        ],
                    );
                }
                st.reveal.replace(Some(handle.clone()));
                // Уровни ещё едут — повторим, когда придут (оригинал получает
                // это бесплатно: эффект строки срабатывает после монтирования)
                if pending && tries < 20 {
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(120));
                        let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeReveal {
                            view,
                            handle,
                            expand_path,
                            select,
                            expand,
                            tries: tries + 1,
                        }));
                    });
                }
            }
            ShellEvent::Tree(TreeEvent::TreeDnd { view, enabled }) => {
                self.trees.entry(view).or_default().dnd = enabled;
            }
            ShellEvent::Tree(TreeEvent::TreeDragStart { view, handle }) => {
                host_link::report_tree(
                    "kamin:tree:handleDrag",
                    vec![serde_json::json!(view), serde_json::json!([handle])],
                );
            }
            ShellEvent::Tree(TreeEvent::TreeDrop { view, handle }) => {
                host_link::report_tree(
                    "kamin:tree:handleDrop",
                    vec![serde_json::json!(view), serde_json::json!(handle)],
                );
            }
            ShellEvent::Tree(TreeEvent::TreeCheckbox {
                view,
                handle,
                state,
            }) => {
                host_link::report_tree(
                    "kamin:tree:reportCheckbox",
                    vec![
                        serde_json::json!(view),
                        serde_json::json!(handle),
                        serde_json::json!(state),
                    ],
                );
            }
            ShellEvent::Ed(EdEvent::SetFileMode(mode)) => {
                self.layout.file_panel_mode = mode.to_string();
                crate::layout_store::save_patch(serde_json::json!({ "filePanelMode": mode }));
                // Вне веб-режима строка адреса не нужна: панель её не рисует.
                if mode != "web" {
                    self.browser_input = None;
                }
            }
            ShellEvent::Ed(EdEvent::OpenFile(path)) => {
                // Открытие файла ГАРАНТИРУЕТ видимую file-панель в режиме
                // Files (запрос юзера): панель включить, Web → Files
                self.reveal_file_panel();
                host_link::request_read_file(self.tx.clone(), path, None);
            }
            ShellEvent::Ed(EdEvent::OpenFileAt(path, line)) => {
                self.reveal_file_panel();
                host_link::request_read_file(self.tx.clone(), path, Some(line));
            }
            ShellEvent::Ed(EdEvent::FileOpened(path, text, target)) => {
                // Успешное чтение снимает прежнюю карточку ошибки
                if !text.is_empty() {
                    self.ed.editor_errors.remove(&path);
                }
                // Уже открыт → просто активировать таб (как оригинал);
                // иначе создать в render (нужен window). target = строка
                // из поиска → goto в render (scroll-to-line).
                if let Some(line) = target {
                    self.ed.pending_goto = Some((path.clone(), line));
                }
                if let Some(i) = self.ed.editor_tabs.iter().position(|t| t.path == path) {
                    self.ed.editor_active = i;
                    self.ed.tabs_reveal_active = true;
                } else {
                    self.ed.pending_editor = Some((path, text));
                }
            }
            ShellEvent::Ed(EdEvent::ToggleFileTabsOverflow) => {
                self.ed.file_tabs_overflow_open = !self.ed.file_tabs_overflow_open;
            }
            ShellEvent::Ed(EdEvent::PushFsUndo(op)) => {
                self.fs_undo.push(op);
                if self.fs_undo.len() > 50 {
                    self.fs_undo.remove(0);
                }
            }
            ShellEvent::Ed(EdEvent::UndoFsOp) => self.undo_fs(),
            ShellEvent::Ed(EdEvent::FsDeletePermanent(path)) => {
                // `fs.delete` оригинала: без корзины и без undo
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let meta = std::fs::metadata(&path);
                    let res = if meta.map(|m| m.is_dir()).unwrap_or(false) {
                        std::fs::remove_dir_all(&path)
                    } else {
                        std::fs::remove_file(&path)
                    };
                    if let Err(e) = res {
                        eprintln!("permanent delete {path} failed: {e}");
                    }
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                });
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
