//! Дерево файлов: листинг каталогов, раскрытие и сворачивание узлов,
//! обновление после изменений на диске.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use crate::ui::file_list::DirEntry;
use gpui::Context;

use crate::root::{RootView, SUB_CLOSE_DELAY_MS};

impl RootView {
    /// Дерево файлов: листинг каталогов, раскрытие и сворачивание узлов.
    pub(crate) fn apply_tree_listing(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Ed(EdEvent::LocateSelectedFile) => {
                // Оригинал (`FileTreeHeader.locateSelected`): цель —
                // `selectedFile.value`, то есть АКТИВНЫЙ ТАБ РЕДАКТОРА, а не
                // выделение дерева (ревью ц.14: приоритет был обратный).
                // Раскрываем предков до корня воркспейса + выделяем строку.
                let target = self
                    .ed
                    .editor_tabs
                    .get(self.ed.editor_active)
                    .map(|t| t.path.clone());
                let target = target.map(|t| t.replace('/', "\\"));
                if let (Some(target), Some(root)) = (target, self.workspace.clone()) {
                    let norm = |s: &str| s.replace('\\', "/").to_lowercase();
                    if norm(&target).starts_with(&norm(&root)) {
                        let mut dir = std::path::PathBuf::from(&target);
                        while let Some(parent) = dir.parent().map(|p| p.to_path_buf()) {
                            let ps = parent.to_string_lossy().to_string();
                            if norm(&ps).len() < norm(&root).len() {
                                break;
                            }
                            let missing = self.tree_mut(cx, |tree| {
                                tree.expanded.insert(ps.clone());
                                !tree.cache.contains_key(&ps)
                            });
                            if missing {
                                host_link::request_list_dir(self.tx.clone(), ps.clone());
                            }
                            if norm(&ps) == norm(&root) {
                                break;
                            }
                            dir = parent;
                        }
                        self.tree_mut(cx, |tree| {
                            tree.selected.clear();
                            tree.selected.insert(target.clone());
                            // `.flash` 0.9 с на найденной строке (ревью ц.18/21)
                            let seq = tree.flash.as_ref().map_or(0, |(_, n)| n + 1);
                            tree.flash = Some((target.clone(), seq));
                        });
                        // `scrollIntoView({ block: "center" })` — строка встаёт
                        // по ЦЕНТРУ видимой области, а не на фикс. 140 px от
                        // верха (ревью ц.18/21). Высоту тела берём из probe.
                        if let Some(idx) =
                            crate::ui::file_list::flat_row_index(self.tree(cx), &root, &target)
                        {
                            const ROW_H: f32 = 22.0;
                            let body_h = crate::probe::registry::bounds_of("tree")
                                .map(|[_, _, _, h]| h)
                                .unwrap_or(400.0);
                            let y = (idx as f32 * ROW_H - (body_h - ROW_H) / 2.0).max(0.0);
                            // Через `tree_mut`: прокрутка — тоже изменение
                            // состояния, панели нужен `notify`, иначе строка
                            // выделена, а списка не сдвинуло (ревью).
                            self.tree_mut(cx, |tree| {
                                tree.scroll
                                    .set_offset(gpui::point(gpui::px(0.0), gpui::px(-y)));
                            });
                        } else {
                            // Листинги предков ещё не пришли — повторим, когда
                            // они появятся (оригинал поллит `[data-tree-id]`
                            // 50 мс ×60, `FileTreeHeader.tsx:85-98`)
                            let tx = self.tx.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(120));
                                let _ = tx.try_send(ShellEvent::Ed(EdEvent::LocateSelectedFile));
                            });
                        }
                    }
                }
            }
            ShellEvent::Ed(EdEvent::FileMenuOpenIn(open)) => {
                // `SUB_CLOSE_DELAY_MS = 250` (`FileContextMenu.tsx:19,35-38`):
                // открытие мгновенное, ЗАКРЫТИЕ с грацией — иначе диагональный
                // проход курсора к каскаду убивал его (ревью ц.13)
                self.file_sub_gen = self.file_sub_gen.wrapping_add(1);
                let sub_gen = self.file_sub_gen;
                if open {
                    if let Some(fm) = self.file_menu.as_mut() {
                        fm.open_in = true;
                    }
                } else {
                    cx.spawn(async move |this, cx| {
                        smol::Timer::after(std::time::Duration::from_millis(SUB_CLOSE_DELAY_MS))
                            .await;
                        let _ = this.update(cx, |this, cx| {
                            if this.file_sub_gen == sub_gen
                                && let Some(fm) = this.file_menu.as_mut()
                            {
                                fm.open_in = false;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
            }
            ShellEvent::Tree(TreeEvent::RefreshTreeHard) => {
                // `workspaceFolder = null` → микротаск → назад: узлы теряют
                // `entries` и `childCap`, раскрытия сохраняются
                self.tree_mut(cx, |tree| {
                    tree.cache.clear();
                    tree.child_cap.clear();
                    tree.loading = tree.expanded.clone();
                });
                if let Some(root) = self.workspace.clone() {
                    host_link::request_list_dir(self.tx.clone(), root);
                }
                for dir in self.tree(cx).expanded.clone() {
                    host_link::request_list_dir(self.tx.clone(), dir);
                }
            }
            ShellEvent::Tree(TreeEvent::RefreshTree) => {
                // Перечитать листинги корня и раскрытых папок. Кэш НЕ чистим:
                // до прихода DirListing дерево рендерилось бы пустым — панель
                // мигала на каждое событие watcher-а (git/сборка/логи).
                if let Some(root) = self.workspace.clone() {
                    host_link::request_list_dir(self.tx.clone(), root);
                }
                for dir in self.tree(cx).expanded.clone() {
                    host_link::request_list_dir(self.tx.clone(), dir);
                }
            }
            ShellEvent::Tree(TreeEvent::CollapseTree) => {
                // Кнопка — ТУМБЛЕР (`toggleCollapseAll`): свернуть всё, кроме
                // корня, либо раскрыть. Раскрытие каскадное: разворачиваем
                // директории с уже известным листингом, а новые уровни
                // добираются в `DirListing`, пока флаг взведён.
                self.tree_all_collapsed = !self.tree_all_collapsed;
                if self.tree_all_collapsed {
                    let root = self.workspace.clone();
                    self.tree_mut(cx, |tree| {
                        tree.expanded.clear();
                        if let Some(root) = root {
                            tree.expanded.insert(root);
                        }
                    });
                } else {
                    self.expand_loaded_dirs(cx);
                }
            }
            ShellEvent::Tree(TreeEvent::DirListing(dir, entries)) => {
                self.tree_mut(cx, |tree| tree.loading.remove(&dir));
                let mut list: Vec<DirEntry> = entries
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|e| {
                                Some(DirEntry {
                                    name: e.get("name")?.as_str()?.to_string(),
                                    is_dir: e.get("type")?.as_str()? == "dir",
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                // Как в kamin-ide: папки первыми, потом по имени
                list.sort_by(|a, b| {
                    b.is_dir
                        .cmp(&a.is_dir)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
                // Декорации (git badge/цвет) для новых путей этого листинга
                let fresh: Vec<String> = list
                    .iter()
                    .map(|e| crate::ui::file_list::join(&dir, &e.name))
                    .filter(|p| !self.tree(cx).deco.contains_key(p))
                    .collect();
                host_link::request_decorations(self.tx.clone(), fresh);
                self.tree_mut(cx, |tree| tree.cache.insert(dir, list));
            }
            ShellEvent::Tree(TreeEvent::ToggleDir(path)) => {
                let known = self.tree(cx).cache.contains_key(&path);
                let expanded = self.tree(cx).expanded.contains(&path);
                let request = !expanded && !known;
                self.tree_mut(cx, |tree| {
                    if expanded {
                        tree.expanded.remove(&path);
                    } else {
                        tree.expanded.insert(path.clone());
                        if request {
                            tree.loading.insert(path.clone());
                        }
                    }
                });
                if request {
                    host_link::request_list_dir(self.tx.clone(), path);
                }
                // Persist раскрытий (кап 500, как tree-expansion.ts)
                let expanded: Vec<&String> = self.tree(cx).expanded.iter().take(500).collect();
                crate::layout_store::save_patch(serde_json::json!({ "treeExpanded": expanded }));
            }
            ShellEvent::Ed(EdEvent::OpenFileMenu(path, is_dir, x, y)) => {
                self.close_popovers_except("file");
                // Мультиселект: кликнутый путь входит в выбор из >1 → Delete по всем
                let multi =
                    if self.tree(cx).selected.len() > 1 && self.tree(cx).selected.contains(&path) {
                        self.tree(cx).selected.iter().cloned().collect()
                    } else {
                        Vec::new()
                    };
                self.file_menu = Some(crate::ui::file_menu::FileMenu {
                    path,
                    is_dir,
                    x,
                    y,
                    open_in: false,
                    multi,
                });
            }
            ShellEvent::Ed(EdEvent::CloseFileMenu) => self.file_menu = None,
            ShellEvent::Ed(EdEvent::FileOpenFailed(path, err)) => {
                self.ed.editor_errors.insert(path.clone(), err);
                // Вкладка всё равно открывается — как в оригинале, где
                // `MonacoEditor` монтируется и сам показывает карточку
                let _ = self.tx.try_send(ShellEvent::Ed(EdEvent::FileOpened(
                    path,
                    String::new(),
                    None,
                )));
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
