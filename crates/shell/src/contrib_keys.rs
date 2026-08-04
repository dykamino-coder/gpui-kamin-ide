//! Contributed keybindings (registry.keybindings): движок сопоставления —
//! VS Code «key» (Windows-поле, "ctrl+shift+p") нормализуется и матчится
//! с gpui Keystroke на root on_key_down; совпадение → kamin:command:execute.
//! when-клаузы: выполняем только пустые (движка when нет; недо-срабатывание
//! безопаснее лже-срабатывания в чужом контексте).

use std::collections::HashMap;

/// Нормализованный вид: "alt+ctrl+shift+win+key" (модификаторы по алфавиту).
pub fn normalize_vscode(key: &str) -> Option<String> {
    let mut mods: Vec<&str> = Vec::new();
    let mut main: Option<String> = None;
    for part in key.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "cmd" | "meta" => mods.push("ctrl"), // cmd-биндинги мапим на ctrl (Windows)
            "shift" => mods.push("shift"),
            "alt" | "option" => mods.push("alt"),
            "win" | "super" => mods.push("win"),
            "" => return None,
            k => main = Some(k.to_string()),
        }
    }
    let main = main?;
    mods.sort_unstable();
    mods.dedup();
    if mods.is_empty() {
        Some(main)
    } else {
        Some(format!("{}+{}", mods.join("+"), main))
    }
}

/// gpui Keystroke → тот же нормализованный вид.
pub fn normalize_keystroke(ks: &gpui::Keystroke) -> String {
    let mut mods: Vec<&str> = Vec::new();
    if ks.modifiers.alt {
        mods.push("alt");
    }
    if ks.modifiers.control {
        mods.push("ctrl");
    }
    if ks.modifiers.shift {
        mods.push("shift");
    }
    if ks.modifiers.platform {
        mods.push("win");
    }
    mods.sort_unstable();
    let key = ks.key.to_ascii_lowercase();
    if mods.is_empty() {
        key
    } else {
        format!("{}+{}", mods.join("+"), key)
    }
}

/// Снапшот registry.keybindings → map нормализованный-key → command.
/// Только записи без when (см. модуль-док).
pub fn build_map(items: &[(String, String, String)]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, command, when) in items {
        if !when.trim().is_empty() {
            continue;
        }
        if let Some(norm) = normalize_vscode(key) {
            map.insert(norm, command.clone());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::{build_map, normalize_vscode};

    #[test]
    fn normalize_orders_modifiers() {
        assert_eq!(
            normalize_vscode("shift+ctrl+p").as_deref(),
            Some("ctrl+shift+p")
        );
        assert_eq!(
            normalize_vscode("Ctrl+Alt+X").as_deref(),
            Some("alt+ctrl+x")
        );
        assert_eq!(normalize_vscode("f5").as_deref(), Some("f5"));
        assert_eq!(normalize_vscode("cmd+k").as_deref(), Some("ctrl+k"));
    }

    #[test]
    fn map_skips_when_clauses() {
        let items = vec![
            ("ctrl+1".into(), "a.cmd".into(), String::new()),
            ("ctrl+2".into(), "b.cmd".into(), "editorFocus".into()),
        ];
        let m = build_map(&items);
        assert_eq!(m.get("ctrl+1").map(String::as_str), Some("a.cmd"));
        assert!(!m.contains_key("ctrl+2"));
    }
}
