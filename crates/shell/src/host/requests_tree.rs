//! Запросы дерева: листинг ФС, декорации, contributed-деревья.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
pub use crate::host::events::ShellEvent;
use crate::host::events::TreeEvent;
use crate::host_link::client;
use serde_json::Value;
use smol::channel::Sender;

/// Листинг директории — читаем ФС НАПРЯМУЮ (нативный Rust-шелл, доступ к диску
/// есть). Не RPC к Node-хосту: тот голодает под индексатором (Q5) → «Loading»
/// минутами; std::fs::read_dir мгновенен (как worktree-скан Zed). Ленивый —
/// только раскрытая папка. Ответ прилетит ShellEvent::Tree(TreeEvent::DirListing).
pub fn request_list_dir(tx: Sender<ShellEvent>, dir: String) {
    std::thread::spawn(move || {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                eprintln!("read_dir {dir} failed: {e}");
                let _ = tx.try_send(ShellEvent::Tree(TreeEvent::DirListing(
                    dir,
                    serde_json::json!([]),
                )));
                return;
            }
        };
        let entries: Vec<Value> = rd
            .filter_map(std::result::Result::ok)
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                // symlink_metadata дешевле stat по цели; is_dir достаточно
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                serde_json::json!({"name": name, "type": if is_dir { "dir" } else { "file" }})
            })
            .collect();
        let _ = tx.try_send(ShellEvent::Tree(TreeEvent::DirListing(
            dir,
            serde_json::json!(entries),
        )));
    });
}

/// Фоновый pull file-decorations (kamin:fileDecoration:get) per path → DecoSet.
pub fn request_decorations(tx: Sender<ShellEvent>, paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let mut out = Vec::with_capacity(paths.len());
        for p in paths {
            if let Ok(v) = client.request("kamin:fileDecoration:get", vec![serde_json::json!(&p)]) {
                out.push((p, v));
            }
        }
        if !out.is_empty() {
            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::DecoSet(out)));
        }
    });
}

/// Фоновый нечёткий поиск файла (kamin:index:findFile) → QuickOpenResults.
pub fn request_find_file(tx: Sender<ShellEvent>, query: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(v) = client.request("kamin:index:findFile", vec![serde_json::json!(query)]) else {
            return;
        };
        let hits = v
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|h| {
                        Some(crate::ui::quick_open::FileHit {
                            rel: h.get("rel")?.as_str()?.to_string(),
                            abs: h.get("abs")?.as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let _ = tx.try_send(ShellEvent::QuickOpenResults(hits));
    });
}

/// Дети уровня contributed-дерева. `parent` пустой = корень.
pub fn request_tree_children(tx: Sender<ShellEvent>, view: String, parent: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let arg_parent = if parent.is_empty() {
            Value::Null
        } else {
            serde_json::json!(parent)
        };
        let nodes = client
            .request(
                "kamin:tree:getChildren",
                vec![serde_json::json!(view), arg_parent],
            )
            .ok()
            .and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(crate::ui::contributed_tree::TreeNodeDto::from_value)
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeChildren {
            view,
            parent,
            nodes,
        }));
    });
}

/// Иконка расширения (`kamin:extensions:icon` → data-URL или null).
/// Кэш держит RootView: один запрос на id, как `iconCache` оригинала.
pub fn request_extension_icon(tx: Sender<ShellEvent>, id: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let url = client
            .request("kamin:extensions:icon", vec![serde_json::json!(id)])
            .ok()
            .and_then(|v| v.as_str().map(str::to_string));
        let _ = tx.try_send(ShellEvent::Cz(CzEvent::ExtensionIcon(id, url)));
    });
}

/// Есть ли у вью DnD-контроллер (`treeHasDnd`): бродкаст `kamin:tree:dnd`
/// уходит в момент регистрации, то есть ДО того, как мы подписались.
pub fn request_tree_dnd(tx: Sender<ShellEvent>, view: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(v) = client.request("kamin:tree:hasDnd", vec![serde_json::json!(view)]) else {
            return;
        };
        if v.as_bool() == Some(true) {
            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeDnd {
                view,
                enabled: true,
            }));
        }
    });
}

/// Мета вью (`createTreeView`: title/description/badge/message).
pub fn request_tree_meta(tx: Sender<ShellEvent>, view: String) {
    std::thread::spawn(move || {
        let Some(client) = client() else { return };
        let Ok(v) = client.request("kamin:tree:getMeta", vec![serde_json::json!(view)]) else {
            return;
        };
        let meta = crate::ui::contributed_tree::TreeMeta::from_value(&v);
        if !meta.is_empty() {
            let _ = tx.try_send(ShellEvent::Tree(TreeEvent::TreeMetaSet { view, meta }));
        }
    });
}

/// Репорты провайдеру: раскрытие/выделение/чекбокс (fire-and-forget).
pub fn report_tree(method: &'static str, args: Vec<Value>) {
    std::thread::spawn(move || {
        if let Some(client) = client() {
            let _ = client.request(method, args);
        }
    });
}
