//! BR-31: per-view delivery trail of the outbox. When host batches wait longer
//! than [`STALL`] with no page pull, one `[outbox]` line tells which step was
//! the last to happen after they arrived: the pull was not requested
//! (`mark`), not sent to the page (`dispatch`), or sent but never fetched
//! (`take`). Nothing here changes delivery.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Batches older than this without a page pull count as a stall.
const STALL: Duration = Duration::from_millis(500);
/// A persisting stall is repeated at most this often per view.
const REPEAT: Duration = Duration::from_secs(5);

#[derive(Default, Clone, Copy)]
struct Trail {
    batches: u32,
    oldest: Option<Instant>,
    mark: Option<Instant>,
    dispatch: Option<Instant>,
    take: Option<Instant>,
    reported: Option<Instant>,
}

static TRAILS: LazyLock<Mutex<HashMap<String, Trail>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn update(id: &str, apply: impl FnOnce(&mut Trail)) {
    if let Ok(mut map) = TRAILS.lock() {
        if !map.contains_key(id) && map.len() >= 64 {
            return;
        }
        apply(map.entry(id.to_string()).or_default());
    }
}

pub(super) fn pushed(id: &str) {
    let now = Instant::now();
    update(id, |t| {
        t.batches += 1;
        t.oldest.get_or_insert(now);
    });
}

pub(super) fn marked(id: &str) {
    update(id, |t| t.mark = Some(Instant::now()));
}

pub(super) fn dispatched(id: &str) {
    update(id, |t| t.dispatch = Some(Instant::now()));
}

pub(super) fn taken(id: &str) {
    update(id, |t| {
        t.batches = 0;
        t.oldest = None;
        t.reported = None;
        t.take = Some(Instant::now());
    });
}

/// Emit the stalls of live views. Called once a second with the other diag lines.
pub(super) fn report(now: Instant) {
    let live = super::browsers::ids();
    let lines = {
        let Ok(mut map) = TRAILS.lock() else { return };
        map.retain(|id, _| live.contains(id));
        stall_lines(now, &mut map)
    };
    for line in lines {
        super::diag_sink::emit_line(line);
    }
}

fn ago(now: Instant, at: Option<Instant>) -> Option<u128> {
    at.map(|at| now.saturating_duration_since(at).as_millis())
}

fn stall_lines(now: Instant, map: &mut HashMap<String, Trail>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut ids: Vec<&String> = map.keys().collect();
    ids.sort();
    let ids: Vec<String> = ids.into_iter().cloned().collect();
    for id in ids {
        let Some(t) = map.get_mut(&id) else { continue };
        let Some(oldest) = t.oldest else { continue };
        if now.saturating_duration_since(oldest) < STALL
            || t.reported
                .is_some_and(|at| now.saturating_duration_since(at) < REPEAT)
        {
            continue;
        }
        t.reported = Some(now);
        lines.push(format!(
            "[outbox] {id:?}: stalled batches={} oldest_ms={} mark_ago_ms={:?} dispatch_ago_ms={:?} take_ago_ms={:?}",
            t.batches,
            now.saturating_duration_since(oldest).as_millis(),
            ago(now, t.mark),
            ago(now, t.dispatch),
            ago(now, t.take),
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trail(oldest_ms: u64, now: Instant) -> Trail {
        Trail {
            batches: 4,
            oldest: Some(now - Duration::from_millis(oldest_ms)),
            mark: Some(now - Duration::from_millis(oldest_ms - 1)),
            dispatch: Some(now - Duration::from_millis(oldest_ms + 300)),
            take: Some(now - Duration::from_millis(oldest_ms + 301)),
            reported: None,
        }
    }

    #[test]
    fn a_stall_names_the_last_step_and_repeats_only_after_five_seconds() {
        let now = Instant::now() + Duration::from_secs(60);
        let mut map = HashMap::from([
            ("stuck".to_string(), trail(800, now)),
            ("fresh".to_string(), trail(100, now)),
            ("empty".to_string(), Trail::default()),
        ]);
        let lines = stall_lines(now, &mut map);
        assert_eq!(
            lines.len(),
            1,
            "only the batch older than 500 ms: {lines:?}"
        );
        assert!(lines[0].starts_with("[outbox] \"stuck\": stalled batches=4 oldest_ms=800"));
        // Marked after the batch arrived, dispatched and taken before it: no dispatch.
        assert!(
            lines[0].contains(
                "mark_ago_ms=Some(799) dispatch_ago_ms=Some(1100) take_ago_ms=Some(1101)"
            )
        );
        let stuck = |lines: Vec<String>| lines.iter().filter(|l| l.contains("\"stuck\"")).count();
        assert_eq!(
            stuck(stall_lines(now + Duration::from_secs(1), &mut map)),
            0
        );
        assert_eq!(
            stuck(stall_lines(now + Duration::from_secs(5), &mut map)),
            1
        );
    }

    #[test]
    fn a_page_pull_clears_the_stall() {
        update("cleared-view", |t| *t = Trail::default());
        pushed("cleared-view");
        taken("cleared-view");
        let mut map = TRAILS.lock().unwrap();
        let t = *map.get("cleared-view").unwrap();
        assert_eq!(
            (t.batches, t.oldest.is_none(), t.take.is_some()),
            (0, true, true)
        );
        let later = Instant::now() + Duration::from_secs(2);
        assert!(
            stall_lines(later, &mut map)
                .iter()
                .all(|line| !line.contains("cleared-view"))
        );
    }
}
