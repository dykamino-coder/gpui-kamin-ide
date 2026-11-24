//! Per-session stashes (file tabs and the Web URL of inactive sessions) live
//! only as long as their session does.
//!
//! An inactive session's file tabs wait in `EditorState::editor_tabs_stash`
//! and its Web URL in `browser_url_stash`; both are taken back only when that
//! session becomes active again. A deleted session can never become active,
//! so its entry (two code editors and two minimaps per file) used to stay
//! until exit: every "new session, open files, delete it" round kept about
//! 7 MB. Invariant: a stash key is a session id present in the latest host
//! snapshot. Until the first snapshot nothing is known and nothing is dropped.

use std::collections::HashMap;

use kamin_model::SessionsSnapshot;

use crate::root::RootView;

/// Whether `id` may keep stashed state: the session still exists, or no
/// snapshot has arrived yet.
pub(crate) fn session_alive(id: &str, snap: Option<&SessionsSnapshot>) -> bool {
    snap.is_none_or(|s| s.sessions.iter().any(|x| x.id == id))
}

/// Drop the entries of sessions that the snapshot no longer lists.
pub(crate) fn retain_alive<V>(stash: &mut HashMap<String, V>, snap: Option<&SessionsSnapshot>) {
    if snap.is_some() {
        stash.retain(|id, _| session_alive(id, snap));
    }
}

impl RootView {
    /// Release the stashed tabs and URLs of deleted sessions.
    pub(crate) fn prune_session_stash(&mut self) {
        let snap = self.sessions.as_ref();
        retain_alive(&mut self.ed.editor_tabs_stash, snap);
        retain_alive(&mut self.browser_url_stash, snap);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snap(ids: &[&str]) -> SessionsSnapshot {
        let sessions: Vec<_> = ids
            .iter()
            .map(|id| {
                json!({"id": id, "name": id, "projectId": "p1", "lastOpened": 1u64,
                       "createdAt": 1u64, "pinned": false, "color": "#89b4fa", "open": false})
            })
            .collect();
        serde_json::from_value(json!({
            "projects": [{"id": "p1", "folderPath": null, "createdAt": 1u64}],
            "sessions": sessions,
            "activeSessionId": null
        }))
        .expect("snapshot shape")
    }

    #[test]
    fn deleted_sessions_lose_their_stash() {
        let mut stash: HashMap<String, u32> =
            [("a".into(), 1), ("b".into(), 2), ("c".into(), 3)].into();
        retain_alive(&mut stash, Some(&snap(&["a", "c"])));
        let mut left: Vec<_> = stash.keys().cloned().collect();
        left.sort();
        assert_eq!(left, ["a", "c"]);
    }

    #[test]
    fn closed_but_listed_sessions_keep_their_stash() {
        // `open: false` (tab closed) is still a listed session: reopening it
        // brings its file tabs back, so its stash stays.
        let mut stash: HashMap<String, u32> = [("a".into(), 1)].into();
        retain_alive(&mut stash, Some(&snap(&["a"])));
        assert_eq!(stash.len(), 1);
    }

    #[test]
    fn nothing_is_dropped_before_the_first_snapshot() {
        let mut stash: HashMap<String, u32> = [("a".into(), 1)].into();
        retain_alive(&mut stash, None);
        assert_eq!(stash.len(), 1);
        assert!(session_alive("a", None));
        assert!(!session_alive("a", Some(&snap(&["b"]))));
    }
}
