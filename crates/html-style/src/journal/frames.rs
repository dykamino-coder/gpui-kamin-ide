//! Work counters of document frames painted in windows.

use super::{enabled, kept_here, with};

/// Count a document painted in a window, including a retained replay.
pub fn window_frame() {
    if enabled() && kept_here() {
        with(|journal| journal.window_frames += 1);
    }
}

/// Count every document build, including retries before and inside draw.
pub fn window_document_build() {
    if enabled() && kept_here() {
        with(|journal| journal.window_document_builds += 1);
    }
}

/// Count a displayed scroll-animation frame and whether it rebuilt its document.
pub fn scroll_animation_frame(built: bool) {
    if enabled() && kept_here() {
        with(|journal| {
            journal.scroll_animation_frames += 1;
            journal.scroll_animation_builds += u64::from(built);
        });
    }
}
