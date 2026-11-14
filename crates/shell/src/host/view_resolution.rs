//! One resolve request per view and extension-host generation, before HTML arrives.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Default)]
pub(crate) struct ViewResolution {
    generation: u64,
    requested: HashSet<String>,
    html: HashMap<String, String>,
}

impl ViewResolution {
    pub(crate) fn is_requested(&self, id: &str) -> bool {
        self.requested.contains(id)
    }

    pub(crate) fn reset(&mut self) {
        self.generation += 1;
        self.requested.clear();
        self.html.clear();
    }

    pub(crate) fn accept_html(&mut self, id: &str, html: &str) -> bool {
        self.requested.insert(id.into());
        if self.html.get(id).is_some_and(|previous| previous == html) {
            return false;
        }
        self.html.insert(id.into(), html.into());
        true
    }
}

/// Tests that drive the process-wide host link (client slot, resolve state) run one at a time.
#[cfg(test)]
pub(crate) fn serial_test() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Claim under the lock, then release it before RPC: HTML may arrive during RPC.
pub(crate) fn request_once(
    state: &Mutex<ViewResolution>,
    id: &str,
    request: impl FnOnce() -> Result<(), String>,
) {
    let generation = {
        let mut state = state.lock().unwrap();
        if !state.requested.insert(id.into()) {
            return;
        }
        state.generation
    };
    if request().is_err() {
        let mut state = state.lock().unwrap();
        // A late failure from the previous child must not reopen the new request.
        // HTML is also proof of success if the RPC response itself timed out.
        if state.generation == generation && !state.html.contains_key(id) {
            state.requested.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_update_burst_before_html_requests_each_view_once() {
        let state = Mutex::new(ViewResolution::default());
        let mut sent = Vec::new();
        for _ in 0..16 {
            for id in ["chat", "plan", "console", "contributed"] {
                request_once(&state, id, || {
                    sent.push(id);
                    Ok(())
                });
            }
        }
        assert_eq!(sent, ["chat", "plan", "console", "contributed"]);
        assert!(state.lock().unwrap().is_requested("chat"));
        state.lock().unwrap().reset();
        request_once(&state, "chat", || {
            sent.push("chat");
            Ok(())
        });
        assert_eq!(sent.len(), 5);
    }

    #[test]
    fn failure_retries_but_late_failure_does_not_clear_new_generation() {
        let state = Mutex::new(ViewResolution::default());
        request_once(&state, "chat", || Err("provider not registered".into()));
        let mut retried = false;
        request_once(&state, "chat", || {
            retried = true;
            state.lock().unwrap().reset();
            request_once(&state, "chat", || Ok(()));
            Err("old child gone".into())
        });
        assert!(retried, "failed request must allow retry");
        request_once(&state, "chat", || panic!("new request was lost"));
    }

    #[test]
    fn concurrent_registry_update_does_not_duplicate_in_flight_request() {
        let state = Mutex::new(ViewResolution::default());
        request_once(&state, "chat", || {
            request_once(&state, "chat", || panic!("duplicate in-flight resolve"));
            state.lock().unwrap().accept_html("chat", "first");
            Err("response timed out after HTML".into())
        });
        request_once(&state, "chat", || panic!("HTML proved resolve succeeded"));
    }

    #[test]
    fn identical_html_is_dropped_per_view_until_respawn() {
        let mut state = ViewResolution::default();
        assert!(state.accept_html("chat", "first"));
        assert!(!state.accept_html("chat", "first"));
        assert!(state.accept_html("plan", "first"));
        assert!(state.accept_html("chat", "changed"));
        state.reset();
        assert!(state.accept_html("chat", "changed"));
    }
}
