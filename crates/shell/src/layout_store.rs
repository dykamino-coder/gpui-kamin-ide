//! layout.json — persist layout (семантика lib.rs layout_get/layout_set:
//! shallow-merge патчей, atomic temp+rename, дебаунс 250ms как layout-autosave).
//! Схема 1:1 kamin-model::LayoutSnapshot (rightPanelSplit НЕ пишется — как в
//! оригинале). Файл живёт в НАШЕМ data-dir рядом с данными хоста.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use kamin_model::{LayoutSnapshot, merge_shallow};
use serde_json::Value;

fn layout_path() -> PathBuf {
    crate::host_link::data_dirs().0.join("layout.json")
}

fn presets_path() -> PathBuf {
    crate::host_link::data_dirs().0.join("layout-presets.json")
}

/// Именованный пресет: имя + полный снапшот layout + default-on-startup.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct LayoutPreset {
    pub name: String,
    pub snapshot: Value,
    #[serde(default)]
    pub default: bool,
}

/// Все пресеты (отсутствие файла → пусто).
pub fn load_presets() -> Vec<LayoutPreset> {
    std::fs::read_to_string(presets_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Перезапись файла пресетов (atomic temp+rename).
pub fn save_presets(presets: &[LayoutPreset]) {
    let path = presets_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(body) = serde_json::to_string_pretty(presets) {
        let _ = std::fs::write(&tmp, body);
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Список строк из layout.json по ключу вне схемы LayoutSnapshot
/// (переживает merge_shallow): treeExpanded, sessionOrder.
pub fn load_string_list(key: &str) -> Vec<String> {
    std::fs::read_to_string(layout_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| {
            v.get(key).and_then(|a| {
                a.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
            })
        })
        .unwrap_or_default()
}

/// Раскрытые пути дерева.
pub fn load_tree_expanded() -> Vec<String> {
    load_string_list("treeExpanded")
}

/// Произвольный вне-схемный ключ layout.json как JSON (activityModel).
pub fn load_raw_key(key: &str) -> Option<Value> {
    std::fs::read_to_string(layout_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get(key).cloned())
}

/// Валидный starred preset, если он есть.
pub(crate) fn load_default_preset() -> Option<LayoutSnapshot> {
    load_presets()
        .into_iter()
        .filter(|preset| preset.default)
        .find_map(|preset| serde_json::from_value(preset.snapshot).ok())
}

/// Единый startup-приоритет: выбор пользователя сильнее состояния сессии,
/// затем идёт последнее локальное состояние и только потом factory default.
pub(crate) fn select_startup_layout(
    default_preset: Option<LayoutSnapshot>,
    session: Option<LayoutSnapshot>,
    stored: Option<LayoutSnapshot>,
) -> LayoutSnapshot {
    default_preset.or(session).or(stored).unwrap_or_default()
}

/// Предварительная загрузка до прихода session snapshot.
pub fn load() -> LayoutSnapshot {
    let stored = std::fs::read_to_string(layout_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<LayoutSnapshot>(&raw).ok());
    select_startup_layout(load_default_preset(), None, stored)
}

static SAVER: Mutex<Option<Sender<Value>>> = Mutex::new(None);
/// Патчи, отправленные сейверу и ещё не записанные на диск.
static PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Shallow-merge патча в layout.json, с дебаунсом 250ms в фоновом потоке.
pub fn save_patch(patch: Value) {
    use std::sync::atomic::Ordering;
    // Отравленный мьютекс восстанавливаем: одна паника сейвера иначе валила
    // КАЖДЫЙ последующий save_patch (аудит персиста).
    let mut guard = SAVER.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        let (tx, rx) = channel::<Value>();
        *guard = Some(tx);
        std::thread::Builder::new()
            .name("kamin-layout-save".into())
            .spawn(move || {
                loop {
                    // Первый патч → копим окно 250ms, мёржим всё накопленное
                    let Ok(first) = rx.recv() else { return };
                    let mut merged = first;
                    let mut merged_count = 1usize;
                    // Кап на окно накопления: при непрерывном потоке патчей
                    // (быстрые тумблеры/раскрытия) запись не случалась ВООБЩЕ,
                    // и выход терял всё окно изменений (аудит персиста).
                    let started = std::time::Instant::now();
                    while started.elapsed() < Duration::from_secs(1) {
                        match rx.recv_timeout(Duration::from_millis(250)) {
                            Ok(next) => {
                                let mut base = merged;
                                merge_shallow(&mut base, &next);
                                merged = base;
                                merged_count += 1;
                            }
                            Err(_) => break,
                        }
                    }
                    write_merged(&merged);
                    PENDING.fetch_sub(merged_count, Ordering::Release);
                }
            })
            .expect("spawn layout saver");
    }
    if let Some(tx) = guard.as_ref() {
        PENDING.fetch_add(1, Ordering::Release);
        let _ = tx.send(patch);
    }
}

/// Дождаться записи всех отложенных патчей (кап ~1.5с). Зовётся на выходе
/// приложения и по probe `flushLayout`: дебаунс терял хвост изменений при
/// закрытии/убийстве процесса — «лейаут открылся не таким, каким оставил»
/// (жалоба юзера; аудит ресайза: «нет flush на выходе»).
pub fn flush_now() {
    use std::sync::atomic::Ordering;
    let deadline = std::time::Instant::now() + Duration::from_millis(1500);
    while PENDING.load(Ordering::Acquire) > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn write_merged(patch: &Value) {
    let path = layout_path();
    // read-modify-write: неизвестные ключи существующего файла сохраняются
    let mut base: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    merge_shallow(&mut base, patch);
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let Ok(text) = serde_json::to_string_pretty(&base) else {
        return;
    };
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marked(width: f64) -> LayoutSnapshot {
        LayoutSnapshot {
            sidebar_width_px: width,
            ..LayoutSnapshot::default()
        }
    }

    #[test]
    fn default_preset_wins_on_startup() {
        let selected = select_startup_layout(
            Some(marked(101.0)),
            Some(marked(202.0)),
            Some(marked(303.0)),
        );
        assert_eq!(selected.sidebar_width_px, 101.0);
    }

    #[test]
    fn session_then_stored_then_factory_are_fallbacks() {
        assert_eq!(
            select_startup_layout(None, Some(marked(202.0)), Some(marked(303.0))).sidebar_width_px,
            202.0
        );
        assert_eq!(
            select_startup_layout(None, None, Some(marked(303.0))).sidebar_width_px,
            303.0
        );
        assert_eq!(
            select_startup_layout(None, None, None).sidebar_width_px,
            LayoutSnapshot::default().sidebar_width_px
        );
    }
}
