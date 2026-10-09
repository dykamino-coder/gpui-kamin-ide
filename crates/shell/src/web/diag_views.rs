//! Bounded per-view state, frame/post age and changed/ten-second snapshots.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct View {
    frame: Option<Instant>,
    post: Option<Instant>,
    alive: bool,
    html: bool,
    tries: u32,
    resolving: Option<Instant>,
}
#[derive(Default)]
struct State {
    views: BTreeMap<String, View>,
    sampled: Option<Instant>,
    last: String,
    emitted: Option<Instant>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

fn update(id: &str, apply: impl FnOnce(&mut View)) {
    if let Ok(mut state) = STATE.lock() {
        if !state.views.contains_key(id) && state.views.len() >= 64 {
            return;
        }
        apply(state.views.entry(id.to_string()).or_default());
    }
}
pub(super) fn frame(id: &str) {
    update(id, |view| view.frame = Some(Instant::now()));
}
pub(super) fn post(id: &str) {
    update(id, |view| view.post = Some(Instant::now()));
}
pub(crate) fn sample_ui(
    alive: &std::collections::HashSet<String>,
    tries: &std::collections::HashMap<String, u32>,
    started: &std::collections::HashMap<String, Instant>,
) {
    let now = Instant::now();
    let Ok(mut state) = STATE.lock() else { return };
    if state
        .sampled
        .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1))
    {
        return;
    }
    state.sampled = Some(now);
    drop(state);
    let ids = super::browsers::ids();
    let html = ids
        .iter()
        .map(|id| (id.clone(), crate::ui::chat_webview::has_html(id)))
        .collect::<BTreeMap<_, _>>();
    let Ok(mut state) = STATE.lock() else { return };
    state.views.retain(|id, _| ids.contains(id));
    for id in ids.into_iter().take(64) {
        let view = state.views.entry(id.clone()).or_default();
        view.alive = alive.contains(&id);
        view.html = html.get(&id).copied().unwrap_or(false);
        view.tries = tries.get(&id).copied().unwrap_or(0);
        view.resolving = started.get(&id).copied();
    }
}
pub(super) fn report(now: Instant) {
    let visible = super::visibility::visible_set();
    let Ok(mut state) = STATE.lock() else { return };
    let status = state
        .views
        .iter()
        .map(|(id, v)| {
            format!(
                "{id:?}: visible={} alive={} html={} resolve_tries={}",
                visible.contains(id),
                v.alive,
                v.html,
                v.tries
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    if status == state.last
        && state
            .emitted
            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(10))
    {
        return;
    }
    state.last = status.clone();
    state.emitted = Some(now);
    let ages = state
        .views
        .iter()
        .map(|(id, v)| {
            format!(
                "{id:?}: frame_ago_ms={:?} post_ago_ms={:?} resolve_elapsed_ms={:?}",
                v.frame
                    .map(|at| now.saturating_duration_since(at).as_millis()),
                v.post
                    .map(|at| now.saturating_duration_since(at).as_millis()),
                v.resolving
                    .map(|at| now.saturating_duration_since(at).as_millis())
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    drop(state);
    super::diag::emit_line(format!("[views] {status}; ages: {ages}"));
}
