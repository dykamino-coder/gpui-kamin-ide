//! Подтяжка данных из ПРОД-KaminIDE (Tauri, `%APPDATA%/studio.dykamino.kaminide`)
//! в стор gpui-порта: host-сессии/проекты (имена, пины, цвета, metadata,
//! пер-сессионный layout) и bridge-конфиг (savedSessionsByToken — «старые
//! чаты», token/serverUrl если свои пусты, base prompt'ы). Прод — ТОЛЬКО
//! ЧТЕНИЕ; свои записи выигрывают при конфликте id (локальное свежее).
//!
//! Пока юзер живёт на двух версиях, прод-стор растёт — мерж повторяется на
//! каждом буте, на котором mtime прод-файла новее сохранённого маркера
//! (`prod-merge.json` в data). Чаты как таковые живут на СЕРВЕРЕ и приходят
//! по conversationId — токен общий, миграция несёт только их СПИСКИ/метаданные.

use std::path::{Path, PathBuf};

fn prod_dir() -> Option<PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let p = PathBuf::from(appdata).join("studio.dykamino.kaminide");
    p.exists().then_some(p)
}

fn read_json(p: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn write_json_atomic(p: &Path, v: &serde_json::Value) {
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension(format!("tmp.{}", std::process::id()));
    if let Ok(text) = serde_json::to_string_pretty(v)
        && std::fs::write(&tmp, text).is_ok()
    {
        let _ = std::fs::rename(&tmp, p);
    }
}

fn mtime_ms(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Union массивов объектов по строковому полю-ключу: свои (`own`) выигрывают,
/// прод-only добавляются. Порядок: свои как были, затем новые из прода.
fn union_by_key(own: &mut Vec<serde_json::Value>, prod: &[serde_json::Value], key: &str) -> usize {
    let have: std::collections::HashSet<String> = own
        .iter()
        .filter_map(|s| s.get(key)?.as_str().map(str::to_string))
        .collect();
    let mut added = 0;
    for s in prod {
        if let Some(id) = s.get(key).and_then(|v| v.as_str())
            && !have.contains(id)
        {
            own.push(s.clone());
            added += 1;
        }
    }
    added
}

/// Дедуп стора сессий: у прод- и gpui-сторов ОДНИ И ТЕ ЖЕ проекты/чаты живут
/// под разными id (каждая сторона генерила свои), поэтому union по id плодит
/// дубли групп в сайдбаре. Схлопываем: проекты — по folderPath (без папки —
/// по имени), сессии — по conversationId из metadata.bridge (иначе по id);
/// из дублей выживает запись с бОльшим lastOpened, projectId переписывается
/// на выжившего. Идемпотентно; зовётся на каждом буте (самолечение).
fn dedup_sessions_store(own_v: &mut serde_json::Value) -> bool {
    let mut changed = false;
    // Проекты: ключ идентичности.
    let proj_key = |p: &serde_json::Value| -> String {
        let folder = p
            .get("folderPath")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .replace('/', "\\")
            .to_lowercase();
        if folder.is_empty() {
            format!("name:{}", p.get("name").and_then(|v| v.as_str()).unwrap_or(""))
        } else {
            format!("folder:{}", folder.trim_end_matches('\\'))
        }
    };
    let mut id_remap: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(projects) = own_v.get_mut("projects").and_then(|v| v.as_array_mut()) {
        let mut kept_by_key: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let before = projects.len();
        projects.retain(|p| {
            let key = proj_key(p);
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            match kept_by_key.get(&key) {
                Some(kept_id) => {
                    id_remap.insert(id, kept_id.clone());
                    false
                }
                None => {
                    kept_by_key.insert(key, id);
                    true
                }
            }
        });
        changed |= projects.len() != before;
    }
    if let Some(sessions) = own_v.get_mut("sessions").and_then(|v| v.as_array_mut()) {
        // Переназначить сессии схлопнутых проектов.
        for s in sessions.iter_mut() {
            let pid = s.get("projectId").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if let Some(new_pid) = id_remap.get(&pid) {
                s["projectId"] = serde_json::json!(new_pid);
                changed = true;
            }
        }
        // Дедуп чатов: conversationId — иначе id.
        let sess_key = |s: &serde_json::Value| -> String {
            s.pointer("/metadata/bridge/conversationId")
                .and_then(|v| v.as_str())
                .map(|c| format!("conv:{c}"))
                .unwrap_or_else(|| {
                    format!("id:{}", s.get("id").and_then(|v| v.as_str()).unwrap_or(""))
                })
        };
        let last_opened =
            |s: &serde_json::Value| s.get("lastOpened").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let mut best: std::collections::HashMap<String, (usize, f64)> = std::collections::HashMap::new();
        for (i, s) in sessions.iter().enumerate() {
            let k = sess_key(s);
            let lo = last_opened(s);
            match best.get(&k) {
                Some((_, prev)) if *prev >= lo => {}
                _ => {
                    best.insert(k, (i, lo));
                }
            }
        }
        let keep: std::collections::HashSet<usize> = best.values().map(|(i, _)| *i).collect();
        if keep.len() != sessions.len() {
            let mut i = 0;
            sessions.retain(|_| {
                let k = keep.contains(&i);
                i += 1;
                k
            });
            changed = true;
        }
    }
    changed
}

fn merge_sessions_file(prod: &Path, own_path: &Path) -> usize {
    let Some(prod_v) = read_json(prod) else { return 0 };
    let mut own_v = read_json(own_path).unwrap_or_else(|| serde_json::json!({}));
    let mut added = 0;
    for field in ["projects", "sessions"] {
        let prod_arr: Vec<serde_json::Value> = prod_v
            .get(field)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if prod_arr.is_empty() {
            continue;
        }
        let own_obj = own_v.as_object_mut().expect("sessions store is an object");
        let entry = own_obj
            .entry(field)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let Some(arr) = entry.as_array_mut() {
            // Прод-сессии приезжают ЗАКРЫТЫМИ: их PTY на этой стороне нет,
            // open=true рисовал бы мёртвые «активные» чипы.
            let mut incoming = prod_arr;
            if field == "sessions" {
                for s in &mut incoming {
                    if let Some(o) = s.as_object_mut() {
                        o.insert("open".into(), serde_json::Value::Bool(false));
                    }
                }
            }
            added += union_by_key(arr, &incoming, "id");
        }
    }
    if added > 0 {
        write_json_atomic(own_path, &own_v);
    }
    added
}

fn merge_bridge_config(prod: &Path, own_path: &Path) -> usize {
    let Some(prod_v) = read_json(prod) else { return 0 };
    let mut own_v = read_json(own_path).unwrap_or_else(|| serde_json::json!({}));
    let mut added = 0;
    // token/serverUrl — только если своих нет (не перетирать вход юзера).
    let own_token_empty = own_v
        .pointer("/config/token")
        .and_then(|v| v.as_str())
        .is_none_or(str::is_empty);
    if own_token_empty && let Some(cfg) = prod_v.get("config") {
        own_v["config"] = cfg.clone();
        added += 1;
    }
    // savedSessionsByToken: union бакетов по conversationId (свои выигрывают).
    if let Some(prod_map) = prod_v.get("savedSessionsByToken").and_then(|v| v.as_object()) {
        if own_v.get("savedSessionsByToken").is_none() {
            own_v["savedSessionsByToken"] = serde_json::json!({});
        }
        let own_map = own_v["savedSessionsByToken"]
            .as_object_mut()
            .expect("savedSessionsByToken is an object");
        for (bucket, sessions) in prod_map {
            let prod_arr: Vec<serde_json::Value> =
                sessions.as_array().cloned().unwrap_or_default();
            let entry = own_map
                .entry(bucket.clone())
                .or_insert_with(|| serde_json::Value::Array(Vec::new()));
            if let Some(arr) = entry.as_array_mut() {
                added += union_by_key(arr, &prod_arr, "conversationId");
            }
        }
    }
    // basePromptByToken: прод-значения только для отсутствующих ключей.
    if let Some(prod_bp) = prod_v.get("basePromptByToken").and_then(|v| v.as_object()) {
        if own_v.get("basePromptByToken").is_none() {
            own_v["basePromptByToken"] = serde_json::json!({});
        }
        let own_bp = own_v["basePromptByToken"]
            .as_object_mut()
            .expect("basePromptByToken is an object");
        for (k, v) in prod_bp {
            if !own_bp.contains_key(k) {
                own_bp.insert(k.clone(), v.clone());
                added += 1;
            }
        }
    }
    if added > 0 {
        write_json_atomic(own_path, &own_v);
    }
    added
}

/// Разовый (пер-изменение прода) мерж на буте, ДО старта kamin-host —
/// host и расширение читают сторы один раз при подъёме.
pub fn run() {
    let Some(prod) = prod_dir() else { return };
    let (data, _) = crate::host::paths::data_dirs();
    let marker_path = data.join("prod-merge.json");
    let marker = read_json(&marker_path).unwrap_or_else(|| serde_json::json!({}));

    let jobs: [(&str, PathBuf, PathBuf); 2] = [
        ("sessions", prod.join("sessions.json"), data.join("sessions.json")),
        (
            "bridgeConfig",
            prod.join("globalStorage")
                .join("dykamino-studio.claude-bridge")
                .join("open-claude-bridge-config.json"),
            data.join("globalStorage")
                .join("dykamino-studio.claude-bridge")
                .join("open-claude-bridge-config.json"),
        ),
    ];
    let mut new_marker = marker.clone();
    for (name, prod_file, own_file) in jobs {
        if !prod_file.exists() {
            continue;
        }
        let mt = mtime_ms(&prod_file);
        let seen = marker.get(name).and_then(|v| v.as_u64()).unwrap_or(0);
        if mt <= seen {
            continue; // прод не менялся с прошлого мержа
        }
        let added = match name {
            "sessions" => merge_sessions_file(&prod_file, &own_file),
            _ => merge_bridge_config(&prod_file, &own_file),
        };
        if added > 0 {
            eprintln!("[migrate] {name}: подтянуто {added} записей из прод-KaminIDE");
        }
        new_marker[name] = serde_json::json!(mt);
    }
    if new_marker != marker {
        write_json_atomic(&marker_path, &new_marker);
    }
    // Самолечащий дедуп СВОЕГО стора — безусловно (первый мерж без него уже
    // успел задвоить проекты по разным id).
    let own_sessions = data.join("sessions.json");
    if let Some(mut own_v) = read_json(&own_sessions)
        && dedup_sessions_store(&mut own_v)
    {
        write_json_atomic(&own_sessions, &own_v);
        eprintln!("[migrate] sessions: дубли схлопнуты (folderPath/conversationId)");
    }
}
