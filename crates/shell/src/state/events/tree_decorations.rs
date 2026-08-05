//! Дерево файлов: декорации (git-бейджи и цвета), метаданные узлов,
//! выделение строк и «показать ещё» у длинных каталогов.
//!
//! Кого сюда зовут — решает `state/events/dispatch.rs`; связку
//! «вариант события → модуль» проверяет `scripts/check_event_routing.py`.

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use crate::host_link::{self, ShellEvent};
use crate::state::fs_ops::fs_paste;
use gpui::Context;

use crate::root::RootView;

impl RootView {
    /// Дерево файлов: декорации (git-бейджи и цвета), метаданные узлов.
    pub(crate) fn apply_tree_decorations(&mut self, event: ShellEvent, cx: &mut Context<Self>) {
        let _ = cx;
        match event {
            ShellEvent::Tree(TreeEvent::ShowMoreDir(dir)) => {
                // Оригинал: childCap += TREE_CHILD_STEP (не «раскрыть всё»)
                self.tree_mut(cx, |tree| {
                    let cap = tree
                        .child_cap
                        .entry(dir)
                        .or_insert(crate::ui::file_list::DIR_RENDER_CAP);
                    *cap += crate::ui::file_list::DIR_RENDER_STEP;
                });
            }
            ShellEvent::Tree(TreeEvent::DecoSet(batch)) => {
                let batch: Vec<_> = batch.into_iter().collect();
                self.tree_mut(cx, |tree| {
                    for (path, v) in batch {
                        let deco = v.as_object().map(|o| crate::ui::file_list::Deco {
                            badge: o.get("badge").and_then(|b| b.as_str()).map(str::to_string),
                            color: o.get("color").and_then(|c| c.as_str()).map(str::to_string),
                            tooltip: o
                                .get("tooltip")
                                .and_then(|t| t.as_str())
                                .map(str::to_string),
                        });
                        tree.deco.insert(path, deco);
                    }
                });
            }
            ShellEvent::Tree(TreeEvent::DecoInvalidate(uris)) => {
                let paths: Vec<String> = match uris {
                    None => {
                        // refresh all: перезапросить всё закэшированное
                        self.tree_mut(cx, |tree| {
                            let keys: Vec<String> = tree.deco.keys().cloned().collect();
                            tree.deco.clear();
                            keys
                        })
                    }
                    Some(list) => {
                        self.tree_mut(cx, |tree| {
                            for p in &list {
                                tree.deco.remove(p);
                            }
                        });
                        list
                    }
                };
                host_link::request_decorations(self.tx.clone(), paths);
            }
            ShellEvent::Cz(CzEvent::IndexStatus(indexing)) => {
                self.tree_mut(cx, |tree| tree.indexing = indexing);
            }
            ShellEvent::Tree(TreeEvent::SelectTreeNode(path, ctrl, shift)) => {
                // `applyClickSelection` (`file-selection.ts:26-47`)
                if ctrl {
                    // Ctrl/Cmd — тоггл; кликнутый путь становится якорем
                    self.tree_mut(cx, |tree| {
                        if !tree.selected.remove(&path) {
                            tree.selected.insert(path.clone());
                        }
                        tree.anchor = Some(path);
                    });
                } else if shift
                    && let Some(anchor) = self.tree(cx).anchor.clone()
                    && let Some(root) = self.workspace.clone()
                {
                    let order = crate::ui::file_list::visible_order(self.tree(cx), &root);
                    let a = order.iter().position(|x| *x == anchor);
                    let b = order.iter().position(|x| *x == path);
                    self.tree_mut(cx, |tree| {
                        if let (Some(a), Some(b)) = (a, b) {
                            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                            tree.selected = order[lo..=hi].iter().cloned().collect();
                        } else {
                            tree.selected.clear();
                            tree.selected.insert(path.clone());
                            tree.anchor = Some(path);
                        }
                    });
                } else {
                    self.tree_mut(cx, |tree| {
                        tree.selected.clear();
                        tree.selected.insert(path.clone());
                        tree.anchor = Some(path);
                    });
                }
            }
            ShellEvent::Ed(EdEvent::FsCut(paths)) => {
                // Дублируем в OS-клипборд (CF_HDROP) — вставится в Проводнике
                let _ = crate::os_clipboard::write_files(&paths);
                self.fs_clipboard = Some((paths, true));
            }
            ShellEvent::Ed(EdEvent::FsCopy(paths)) => {
                let _ = crate::os_clipboard::write_files(&paths);
                self.fs_clipboard = Some((paths, false));
            }
            ShellEvent::Ed(EdEvent::FsPaste(target_dir)) => {
                // Внутренний буфер пуст → пробуем CF_HDROP из Проводника
                if self.fs_clipboard.is_none() {
                    let os_files = crate::os_clipboard::read_files();
                    if !os_files.is_empty() {
                        let tx = self.tx.clone();
                        std::thread::spawn(move || {
                            for src in os_files {
                                match fs_paste(&src, &target_dir, false) {
                                    Ok(dst) => {
                                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                            crate::host_link::FsUndo::Paste {
                                                dst: dst.display().to_string(),
                                                cut_src: None,
                                            },
                                        )));
                                    }
                                    Err(e) => {
                                        eprintln!("paste (os) failed: {e}");
                                        let _ = tx.try_send(ShellEvent::Toast(
                                            crate::ui::toasts::Toast {
                                                id: format!("paste-{src}"),
                                                severity: "error".into(),
                                                title: None,
                                                message: format!("Вставка не удалась: {e}"),
                                                actions: Vec::new(),
                                                sticky: false,
                                            },
                                        ));
                                    }
                                }
                            }
                            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                        });
                        return;
                    }
                }
                if let Some((srcs, is_cut)) = self.fs_clipboard.clone() {
                    if is_cut {
                        self.fs_clipboard = None; // cut одноразовый
                    }
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        for src in srcs {
                            match fs_paste(&src, &target_dir, is_cut) {
                                Ok(dst) => {
                                    let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                        crate::host_link::FsUndo::Paste {
                                            dst: dst.display().to_string(),
                                            cut_src: is_cut.then_some(src),
                                        },
                                    )));
                                }
                                Err(e) => {
                                    eprintln!("paste failed: {e}");
                                    let _ =
                                        tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                                            id: format!("paste2-{src}"),
                                            severity: "error".into(),
                                            title: None,
                                            message: format!("Вставка не удалась: {e}"),
                                            actions: Vec::new(),
                                            sticky: false,
                                        }));
                                }
                            }
                        }
                        let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                    });
                } else {
                    let _ = self
                        .tx
                        .try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                            id: "paste-empty".into(),
                            severity: "info".into(),
                            title: None,
                            message: "Clipboard is empty".into(),
                            actions: Vec::new(),
                            sticky: false,
                        }));
                }
            }
            ShellEvent::Ed(EdEvent::FsDropMove { dest, paths, copy }) => {
                // Drop на папку (plan/99 п.34): свой драг = MOVE, внешний из
                // ОС = COPY. fs_paste: rename/копия + cross-device фолбэк +
                // guard «папку в саму себя». MOVE в СВОЙ каталог — no-op
                // (без него dst==src уходил в ветку «name copy»).
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    for src in paths {
                        let same_dir = std::path::Path::new(&src)
                            .parent()
                            .is_some_and(|par| par == std::path::Path::new(&dest));
                        if !copy && (same_dir || src == dest) {
                            continue;
                        }
                        match fs_paste(&src, &dest, !copy) {
                            Ok(dst) => {
                                let _ = tx.try_send(ShellEvent::Ed(EdEvent::PushFsUndo(
                                    crate::host_link::FsUndo::Paste {
                                        dst: dst.display().to_string(),
                                        cut_src: (!copy).then_some(src),
                                    },
                                )));
                            }
                            Err(e) => {
                                eprintln!("drop-move failed: {e}");
                                let _ = tx.try_send(ShellEvent::Toast(crate::ui::toasts::Toast {
                                    id: format!("dropmove-{src}"),
                                    severity: "error".into(),
                                    title: None,
                                    message: format!("Перемещение не удалось: {e}"),
                                    actions: Vec::new(),
                                    sticky: false,
                                }));
                            }
                        }
                    }
                    let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
                });
            }
            ShellEvent::Cz(CzEvent::ToggleProblemsFile(uri)) => {
                if !self.problems_collapsed.remove(&uri) {
                    self.problems_collapsed.insert(uri);
                }
            }
            ShellEvent::Cz(CzEvent::ProblemsShowMore) => {
                self.problems_file_cap += 200;
            }
            ShellEvent::Tree(TreeEvent::TreeChildren {
                view,
                parent,
                nodes,
            }) => {
                self.trees
                    .entry(view)
                    .or_default()
                    .levels
                    .insert(parent, Some(nodes));
            }
            ShellEvent::Tree(TreeEvent::TreeMetaSet { view, meta }) => {
                self.trees.entry(view).or_default().meta = meta;
            }
            ShellEvent::Tree(TreeEvent::TreeChanged(view)) => {
                // Рефреш от провайдера: перечитываем корень и все раскрытые
                // уровни (оригинал перемонтирует TreeLevel по version).
                if let Some(st) = self.trees.get(&view) {
                    // Старое содержимое остаётся на экране до прихода нового —
                    // оригинал не сбрасывает состояние уровня (ревью ц.7)
                    let parents: Vec<String> = st.levels.keys().cloned().collect();
                    for parent in parents {
                        host_link::request_tree_children(self.tx.clone(), view.clone(), parent);
                    }
                }
            }
            // Сюда диспетчер чужого не пришлёт
            _ => {}
        }
    }
}
