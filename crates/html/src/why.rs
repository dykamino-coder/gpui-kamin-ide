//! DevTools `why` domain of the engine: why a frame was built instead of
//! replayed, why a write took the slow path, why a cache missed, and who
//! requested each frame. A protocol surface (schema [`SCHEMA`], codes
//! [`CODES`]): readers [`subscribe`], [`take`] what was recorded between
//! frames and send it as [`to_json`]. Off while nobody subscribes; then a
//! record point costs one relaxed load and computes nothing.
//!
//! The same journal serves the future script engine: a script's layout read
//! that forces a frame or a style recalc records its cause here, with the
//! engine line that decided it.

use gpui::dirty_sources::trace;
use serde_json::{Value, json};

pub use gpui::dirty_sources::trace::Event as FrameEvent;
pub use kamin_style::journal::{CacheStats, Cause, Journal, SCHEMA, caches_held, enabled};
pub(crate) use kamin_style::journal::{note, note_at, stage};

/// Stable codes of the protocol (renamed only with a schema bump): causes,
/// cache miss reasons and frame events.
pub const CODES: &[&str] = &[
    // Frame events (`frames`): requests of a frame and the work they led to.
    "notify",
    "notify-deferred",
    "refresh",
    "refresh-all",
    "next-frame",
    "draw",
    "render",
    "layout_retry",
    // Scroll node pairing (gpui `window/scroll_nodes.rs`).
    "scroll_node.open_anchor",
    "scroll_node.paint_paired",
    "scroll_node.paint_unpaired",
    "scroll_node.anchor_unpainted",
    // Cache miss reasons.
    "new_key",
    "evicted",
    "not_admitted",
    "uncacheable",
    // Causes beyond `frame_miss.*` and `paint_path.refused_*` (`codes` below).
    "damage.value",
    "frame_unpublished",
    "layout_retry.container_size",
    "layout_retry.flow_refused",
    "layout_retry.replay_refused",
    "no_layer.painted_environment",
    "no_layer.painted_ink",
    "no_layer.painted_nested",
    "no_layer.painted_other",
    "no_layer.dropped",
    "offscreen.unknown_area",
    "offscreen.on_screen",
    "no_layer.unrecorded",
    "no_layer.frame_unrecorded",
    "not_own",
    "plan_text.refused",
    "restyle.stage",
    "scroll_extent.layout_bars",
    "scroll_extent.relayout",
];

/// Whether `code` belongs to the protocol: listed, or a frame miss or a
/// paint-path refusal (their counter names, `frame_miss.<name>`).
pub fn is_code(code: &str) -> bool {
    CODES.contains(&code)
        || crate::document::frame_miss_code(code)
        || code.strip_prefix("paint_path.").is_some_and(|name| {
            name.starts_with("refused_") && crate::paint_path::NAMES.contains(&name)
        })
}

/// A reader: the journal and the frame trace record while one lives.
#[must_use = "the journal records only while the subscription lives"]
pub struct Subscription {
    _journal: kamin_style::journal::Subscription,
    _frames: trace::Trace,
}

/// Start recording for a new reader.
pub fn subscribe() -> Subscription {
    Subscription {
        _journal: kamin_style::journal::subscribe(),
        _frames: trace::Trace::start(),
    }
}

/// What was recorded since the last take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub journal: Journal,
    /// Requests of frames, draws and renders, in order.
    pub frames: Vec<FrameEvent>,
    /// Frame events past the bound, not kept.
    pub frames_dropped: u64,
}

/// Everything recorded since the last take.
pub fn take() -> Entry {
    let (frames, frames_dropped) = trace::take();
    Entry {
        journal: kamin_style::journal::take(),
        frames,
        frames_dropped,
    }
}

fn site(site: &std::panic::Location<'_>) -> String {
    format!("{}:{}", site.file().replace('\\', "/"), site.line())
}

fn cause(c: &Cause) -> Value {
    json!({"code": c.code, "site": c.site(), "detail": c.detail})
}

/// The protocol form of an entry (schema [`SCHEMA`]).
pub fn to_json(entry: &Entry) -> Value {
    let j = &entry.journal;
    json!({
        "schema": SCHEMA,
        "causes": j.causes.iter().map(cause).collect::<Vec<_>>(),
        "causes_dropped": j.dropped,
        "caches": j.caches.iter().map(|(name, c)| ((*name).to_string(), json!({
            "hits": c.hits,
            "misses": c.misses,
            "examples": c.examples.iter().map(cause).collect::<Vec<_>>(),
        }))).collect::<serde_json::Map<_, _>>(),
        "frames": entry.frames.iter().map(|e| json!({"kind": e.kind, "site": site(e.site), "detail": e.detail})).collect::<Vec<_>>(),
        "frames_dropped": entry.frames_dropped,
    })
}

/// A test's reader of the notes made on its thread.
#[cfg(test)]
pub(crate) struct TestReader {
    _journal: kamin_style::journal::ThreadSubscription,
    _frames: trace::Trace,
}

/// Serializes the tests of this crate that subscribe, each reading the notes
/// of its own thread: tests running beside it note into the same journal.
#[cfg(test)]
pub(crate) fn test_reader() -> (std::sync::MutexGuard<'static, ()>, TestReader) {
    static ONE: trace::Shared<()> = trace::Shared::new(());
    let guard = ONE.lock();
    let sub = TestReader {
        _journal: kamin_style::journal::subscribe_thread(),
        _frames: trace::Trace::start(),
    };
    take();
    (guard, sub)
}

#[cfg(test)]
mod tests;
