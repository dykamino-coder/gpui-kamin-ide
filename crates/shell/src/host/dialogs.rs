//! Нативные диалоги файлов для хост-запросов `shell.showOpenDialog` /
//! `shell.showSaveDialog` (скрепка чата, «Download session log» и т.п.).
//!
//! Форма ответа — как у kamin-ide (`renderer/signals/ipc.ts`): open →
//! массив абсолютных путей или null при отмене; save → путь или null.
//! Зовётся с ФОНОВОГО потока: rfd на Windows сам крутит своё окно.

use serde_json::Value;

/// Общие поля опций VS Code-диалога.
fn apply_common(mut d: rfd::FileDialog, options: &Value) -> rfd::FileDialog {
    if let Some(title) = options.get("title").and_then(Value::as_str) {
        d = d.set_title(title);
    }
    if let Some(path) = options.get("defaultPath").and_then(Value::as_str) {
        let p = std::path::Path::new(path);
        if p.is_dir() {
            d = d.set_directory(p);
        } else {
            if let Some(dir) = p.parent().filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            if let Some(name) = p.file_name() {
                d = d.set_file_name(name.to_string_lossy());
            }
        }
    }
    if let Some(filters) = options.get("filters").and_then(Value::as_object) {
        for (name, exts) in filters {
            let exts: Vec<String> = exts
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            if !exts.is_empty() {
                d = d.add_filter(name, &exts);
            }
        }
    }
    d
}

/// `vscode.window.showOpenDialog` → массив путей или null (отмена).
pub(crate) fn show_open_dialog(options: &Value) -> Value {
    let d = apply_common(rfd::FileDialog::new(), options);
    let many = options
        .get("canSelectMany")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let folders = options
        .get("canSelectFolders")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let picked: Option<Vec<std::path::PathBuf>> = if folders {
        if many {
            d.pick_folders()
        } else {
            d.pick_folder().map(|p| vec![p])
        }
    } else if many {
        d.pick_files()
    } else {
        d.pick_file().map(|p| vec![p])
    };
    match picked {
        Some(paths) => Value::Array(
            paths
                .into_iter()
                .map(|p| Value::String(p.display().to_string()))
                .collect(),
        ),
        None => Value::Null,
    }
}

/// `vscode.window.showSaveDialog` → путь или null (отмена).
pub(crate) fn show_save_dialog(options: &Value) -> Value {
    match apply_common(rfd::FileDialog::new(), options).save_file() {
        Some(p) => Value::String(p.display().to_string()),
        None => Value::Null,
    }
}
