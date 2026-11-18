//! Page frames shown without a window draw (INC-2026-0064).
//!
//! A web view paints its page as a sprite of one atlas tile per view
//! (`shared_texture.rs`). A new page frame of the same size changes only the
//! pixels behind that tile: copying it into the view's own texture and
//! reporting the repainted rectangle is enough, and the window presents its
//! last scene again (`Window::request_present`), redrawing only that
//! rectangle. The root view is not notified, so nothing is laid out or painted.
//!
//! Chromium does the same for an embedded surface: a child compositor frame
//! of the same `SurfaceId` reaches the display compositor directly, and the
//! embedding layer tree commits only when the surface id (its size) changes
//! (`cc/layers/surface_layer.cc`, `viz SurfaceAggregator`).
//!
//! Anything else falls back to a window draw: a frame of another size or
//! region, a view whose last paint was not a 1:1 frame (resize catch-up,
//! stretch, page popup), software rendering and views never painted.

use super::shared_texture::FrameKey;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

/// What a web view painted last: the frame it sampled and the physical size
/// it asked for. `None` means its paint depends on more than the frame.
static PAINTED: LazyLock<Mutex<HashMap<String, Painted>>> = LazyLock::new(Default::default);

/// Views that received a page frame since the pump last looked.
static PENDING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Painted {
    key: FrameKey,
    want: (i32, i32),
}

/// CEF delivered a page frame of view `id` (any thread).
pub(crate) fn arrived(id: &str) {
    if let Ok(mut pending) = PENDING.lock() {
        pending.insert(id.to_string());
    }
    super::diag_why::tick("wake/page-frame");
    super::wake_pump();
}

/// The web view painted frame `key` 1:1 for the physical size `want`, and
/// nothing else of its paint depends on the frame. `None`: it did not.
pub(crate) fn painted(id: &'static str, key: Option<FrameKey>, want: (i32, i32)) {
    if let Ok(mut painted) = PAINTED.lock() {
        match key {
            Some(key) => {
                painted.insert(id.to_string(), Painted { key, want });
            }
            None => {
                painted.remove(id);
            }
        }
    }
}

/// Forget a closed view.
pub(crate) fn forget(id: &str) {
    if let Ok(mut painted) = PAINTED.lock() {
        painted.remove(id);
    }
    if let Ok(mut pending) = PENDING.lock() {
        pending.remove(id);
    }
}

/// Take the views with new page frames.
pub(crate) fn take_pending() -> Vec<String> {
    PENDING
        .lock()
        .map(|mut pending| pending.drain().collect())
        .unwrap_or_default()
}

/// Copy the new frames of `ids` and report their damage. Returns whether all
/// of them are shown by presenting the last scene again; `false` asks for a
/// window draw (their paint then finds the frames copied already).
pub(crate) fn accept(ids: &[String], window: &gpui::Window) -> bool {
    let mut all = true;
    for id in ids {
        let last = PAINTED.lock().ok().and_then(|p| p.get(id.as_str()).copied());
        let Some(last) = last else {
            all = false;
            continue;
        };
        let shown = super::shared_texture::texture_for(id, window, last.want).is_some()
            && super::shared_texture::shown_key(id) == Some(last.key);
        all &= shown;
    }
    super::shared_texture::submit_copies(window);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_views_are_taken_once() {
        arrived("page-frame-test-a");
        arrived("page-frame-test-a");
        let taken = take_pending();
        assert_eq!(
            taken
                .iter()
                .filter(|id| id.as_str() == "page-frame-test-a")
                .count(),
            1
        );
        assert!(!take_pending().iter().any(|id| id == "page-frame-test-a"));
    }

    #[test]
    fn a_view_not_painted_one_to_one_is_forgotten() {
        painted("page-frame-test-b", None, (10, 10));
        assert!(
            PAINTED
                .lock()
                .is_ok_and(|p| !p.contains_key("page-frame-test-b"))
        );
    }
}
