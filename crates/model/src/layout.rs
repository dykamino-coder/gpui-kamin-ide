//! LayoutSnapshot — схема layout.json 1:1 (plan/50 §4, сверено с реальным файлом).
//! ⚠ rightPanelSplit НАМЕРЕННО отсутствует: тип в kamin-ide его объявляет,
//! но capture/apply не пишут — в layout.json его нет (воспроизводим как есть).
//! Патчи — shallow-merge на уровне JSON (merge_shallow), не типизированные.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActivitySlot {
    pub pinned: Vec<String>,
    pub active: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutSnapshot {
    pub sidebar_visible: bool,
    pub sidebar_width_px: f64,
    pub file_panel_visible: bool,
    /// "files" | "web"
    pub file_panel_mode: String,
    pub file_panel_width_ratio: f64,
    pub file_panel_bottom_visible: bool,
    pub file_panel_bottom_height_ratio: f64,
    pub right_panel_visible: bool,
    pub right_panel_width_px: f64,
    pub right_panel_bottom_visible: bool,
    pub main_visible: bool,
    pub main_bottom_visible: bool,
    pub main_split: f64,
    /// Только в глобальном layout.json (пер-сессионный снапшот его не содержит)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_choice: Option<String>,
    pub activity_sidebar: ActivitySlot,
    pub activity_right_top: ActivitySlot,
    pub activity_right_bottom: ActivitySlot,
    pub activity_central_top: ActivitySlot,
    pub activity_central_bottom: ActivitySlot,
    pub activity_main: ActivitySlot,
    pub activity_main_bottom: ActivitySlot,
}

impl Default for LayoutSnapshot {
    fn default() -> Self {
        // DEFAULT_LAYOUT_SNAPSHOT — ДОСЛОВНО из src/config/default-layout.ts
        // (заводской вид свежей установки; слабее stored layout и user-пресета).
        fn slot(pinned: &[&str], active: Option<&str>) -> ActivitySlot {
            ActivitySlot {
                pinned: pinned.iter().map(|s| s.to_string()).collect(),
                active: active.map(String::from),
            }
        }
        Self {
            sidebar_visible: true,
            sidebar_width_px: 241.0,
            file_panel_visible: true,
            file_panel_mode: "files".into(),
            file_panel_width_ratio: 0.478,
            file_panel_bottom_visible: true,
            file_panel_bottom_height_ratio: 0.449,
            right_panel_visible: true,
            right_panel_width_px: 415.0,
            right_panel_bottom_visible: true,
            main_visible: true,
            main_bottom_visible: false,
            main_split: 0.66,
            theme_choice: None,
            activity_sidebar: slot(&["projects"], Some("projects")),
            activity_right_top: slot(&["tree"], Some("tree")),
            activity_right_bottom: slot(
                &[
                    "claudeBridgePlan",
                    "claudeBridgeTodos",
                    "claudeBridgeAgents",
                ],
                Some("claudeBridgePlan"),
            ),
            activity_central_top: slot(&[], None),
            activity_central_bottom: slot(&["claudeBridgeConsole"], Some("claudeBridgeConsole")),
            activity_main: slot(&["claudeBridge"], Some("claudeBridge")),
            activity_main_bottom: slot(&[], None),
        }
    }
}

/// Shallow-merge патча в JSON-объект (семантика layout_set в lib.rs Tauri).
pub fn merge_shallow(base: &mut Value, patch: &Value) {
    let (Value::Object(base), Value::Object(patch)) = (base, patch) else {
        return;
    };
    for (k, v) in patch {
        base.insert(k.clone(), v.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn real_layout_json_roundtrip() {
        // Форма снята с живого %APPDATA%/studio.dykamino.kaminide/layout.json
        let raw = json!({
            "sidebarVisible": true, "sidebarWidthPx": 200,
            "filePanelVisible": true, "filePanelMode": "files",
            "filePanelWidthRatio": 0.6, "filePanelBottomVisible": true,
            "filePanelBottomHeightRatio": 0.3079710144927536,
            "rightPanelVisible": true, "rightPanelWidthPx": 363,
            "rightPanelBottomVisible": true, "mainVisible": true,
            "mainBottomVisible": true, "mainSplit": 0.6643545279383428,
            "themeChoice": "dark",
            "activitySidebar": {"pinned": ["projects","extensions"], "active": "projects"},
            "activityRightTop": {"pinned": ["tree"], "active": "tree"},
            "activityRightBottom": {"pinned": [], "active": null},
            "activityCentralTop": {"pinned": [], "active": null},
            "activityCentralBottom": {"pinned": ["terminal"], "active": "terminal"},
            "activityMain": {"pinned": ["claudeBridge"], "active": "claudeBridge"},
            "activityMainBottom": {"pinned": [], "active": null}
        });
        let snap: LayoutSnapshot = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(snap.sidebar_width_px, 200.0);
        assert_eq!(snap.activity_main.active.as_deref(), Some("claudeBridge"));
        // Сериализация не добавляет rightPanelSplit
        let out = serde_json::to_value(&snap).unwrap();
        assert!(out.get("rightPanelSplit").is_none());
        assert_eq!(out.get("themeChoice"), Some(&json!("dark")));
    }

    #[test]
    fn shallow_merge_replaces_whole_keys() {
        let mut base = json!({"a": 1, "slot": {"pinned": ["x"], "active": "x"}});
        merge_shallow(
            &mut base,
            &json!({"slot": {"pinned": [], "active": null}, "b": 2}),
        );
        assert_eq!(base["a"], 1);
        assert_eq!(base["b"], 2);
        // shallow: вложенный объект замещается целиком, не мёржится
        assert_eq!(base["slot"], json!({"pinned": [], "active": null}));
    }
}
