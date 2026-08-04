//! Обработка событий: События, не попавшие в остальные домены.
//!
//! Вызывается диспетчером `state/events/dispatch.rs` — он и решает,
//! чьё это событие, по варианту `ShellEvent`. Тела армов перенесены
//! из `root.rs` дословно.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host_link::{self, ShellEvent};
use crate::ui::file_list::TreeState;
use gpui::Context;

use crate::root::{RootView, TabDrag, ToolDrag};

impl RootView {
    /// События, не попавшие в остальные домены.
    pub(crate) fn apply_misc(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Workspace(path) => {
                // Корень приводим к АБСОЛЮТНОМУ пути: хост может отдать
                // относительный («.» при запуске без папки), и тогда пути
                // дерева («.\PLAN.md») перестают совпадать с путями табов
                // редактора — ломаются выделение, Locate и декорации
                let path = path.map(|p| {
                    std::path::Path::new(&p)
                        .canonicalize()
                        .map(|c| {
                            c.to_string_lossy()
                                // `canonicalize` на Windows отдаёт UNC-форму
                                // `\\?\C:\…` — её надо снять, иначе пути дерева
                                // не совпадают с путями табов редактора
                                .trim_start_matches(r"\\?\")
                                .to_string()
                        })
                        .unwrap_or(p)
                });
                // Активация: дерево пере-рутится мгновенно, листинги ленивые
                if self.workspace != path {
                    self.tree_mut(cx, |tree| *tree = TreeState::default());
                }
                self.workspace = path.clone();
                // Watcher: авто-RefreshTree на изменения ФС (дебаунс 300ms)
                crate::fs_watch::watch(path.clone(), self.tx.clone());
                if let Some(root) = path {
                    // Корень раскрыт по умолчанию (initiallyExpanded) +
                    // восстановление persist-раскрытий этого воркспейса.
                    let saved = crate::layout_store::load_tree_expanded();
                    let requests = self.tree_mut(cx, |tree| {
                        tree.expanded.insert(root.clone());
                        let mut requests = Vec::new();
                        for p in saved {
                            if p.starts_with(&root) {
                                if !tree.cache.contains_key(&p) && p != root {
                                    requests.push(p.clone());
                                }
                                tree.expanded.insert(p);
                            }
                        }
                        requests
                    });
                    for p in requests {
                        host_link::request_list_dir(self.tx.clone(), p);
                    }
                    host_link::request_list_dir(self.tx.clone(), root);
                }
            }
            ShellEvent::TitlebarDevtools => {
                // Инструменты разработчика для страницы чата.
                crate::web::open_devtools(crate::ui::chat_webview::CHAT_VIEW_ID);
                if !self.cz.customize_open {
                    let _ = self.tx.try_send(ShellEvent::Cz(CzEvent::ToggleCustomize));
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Cz(CzEvent::SetCustomizePanel("system")));
                } else {
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Cz(CzEvent::SetCustomizePanel("system")));
                }
            }
            ShellEvent::Ed(EdEvent::GotoLine(path, line)) => {
                self.ed.pending_goto = Some((path, line));
            }
            ShellEvent::MoveToolTo(src, id, dst) => {
                self.tool_tab_menu = None;
                // Клик по «чужой» одиночке в tool_picker тоже шлёт MoveToolTo —
                // поповер после переноса закрываем (для drag-пути это no-op).
                self.tool_picker = None;
                self.activity.move_activity(src, &id, dst, usize::MAX);
                self.save_activity();
            }
            ShellEvent::UnpinTool(slot, id) => {
                self.activity.unpin(slot, &id);
                self.save_activity();
                self.tool_picker = None;
            }
            ShellEvent::Ed(EdEvent::TabPress(i, x, y)) => {
                self.ed.tab_drag = Some(TabDrag {
                    src: i,
                    start: (x, y),
                    started: false,
                    over: None,
                });
            }
            ShellEvent::ToolPress(slot, id, x, y) => {
                self.tool_drag = Some(ToolDrag {
                    src: slot,
                    id,
                    start: (x, y),
                    pos: (x, y),
                    started: false,
                    over: None,
                    over_index: None,
                });
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
