//! Модель сессий/проектов — зеркало host sessions.json (plan/50 §5,
//! формы сверены с живым файлом).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::layout::LayoutSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    /// null = «No folder»
    pub folder_path: Option<String>,
    pub created_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct OpenFile {
    pub path: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct EditorState {
    pub open_files: Vec<OpenFile>,
    pub active_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    pub project_id: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    /// true = активный таб; false = группа «N inactive»
    #[serde(default)]
    pub open: bool,
    pub last_opened: f64,
    pub created_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_state: Option<EditorState>,
    /// Пер-сессионный layout (снапшот минус themeChoice)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    /// Сим Бриджа: {bridge:{conversationId}, nameSetByUser} — непрозрачно
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Кадр события sessions:changed + ответ sessions:list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionsSnapshot {
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
    pub active_session_id: Option<String>,
    #[serde(default)]
    pub bridge_imported: bool,
}

/// Цвета сессий (SESSION_COLORS, plan/50 §1) — валидируются при имплементации UI.
pub const SESSION_COLORS: [&str; 8] = [
    "#89b4fa", "#a6e3a1", "#f9e2af", "#f38ba8", "#cba6f7", "#94e2d5", "#fab387", "#f5c2e7",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn real_session_shape_roundtrip() {
        let raw = json!({
            "projects": [{"id": "p1", "folderPath": null, "createdAt": 1782129605949u64}],
            "sessions": [{
                "id": "s1", "name": "Session 1", "projectId": "p1",
                "lastOpened": 1784763898208u64, "createdAt": 1782129605949u64,
                "pinned": false, "color": "#94e2d5", "open": true,
                "editorState": {"openFiles": [{"path": "C:/w/a.ts", "pinned": true}], "activeFile": "C:/w/a.ts"},
                "webUrl": "https://example.com",
                "metadata": {"bridge": {"conversationId": "abc"}, "nameSetByUser": true}
            }],
            "activeSessionId": "s1",
            "bridgeImported": true
        });
        let snap: SessionsSnapshot = serde_json::from_value(raw).unwrap();
        assert_eq!(snap.projects[0].folder_path, None);
        let s = &snap.sessions[0];
        assert!(s.open);
        assert_eq!(
            s.editor_state.as_ref().unwrap().open_files[0].path,
            "C:/w/a.ts"
        );
        assert_eq!(
            s.metadata.as_ref().unwrap()["bridge"]["conversationId"],
            json!("abc")
        );
    }

    #[test]
    fn minimal_session_parses_with_defaults() {
        let s: Session = serde_json::from_value(json!({
            "id": "x", "name": "n", "projectId": "p",
            "lastOpened": 1.0, "createdAt": 1.0
        }))
        .unwrap();
        assert!(!s.open);
        assert!(!s.pinned);
        assert!(s.layout.is_none());
    }
}
