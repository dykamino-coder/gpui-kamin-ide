//! Scroll mode: the last wheel-step frame (built or replayed scrolled) must
//! equal a fresh document opened at the same root scroll offset. A replay
//! that draws stale content (a stationary layer that should scroll, a moved
//! copy without its deferred draws) fails the run instead of only timing it.

use super::frames::Run;
use super::run::{Step, sample};
use gpui::frame_capture::CapturedFrame;
use serde_json::{Value, json};
use std::rc::Rc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The last scroll frame and the root offset it was drawn at.
pub(super) type LastFrame = Option<(Rc<CapturedFrame>, (f32, f32))>;

pub(super) async fn check(last: LastFrame, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
    let (cx, window, fonts, args, capture, luid) = run;
    let Some((incremental, offset)) = last else {
        return Err("scroll mode captured no frame to check against a fresh document".into());
    };
    window.update(cx, |view, _, _| view.fresh_scroll = Some(offset))?;
    let (_, fresh, _) = sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
    let at = window.read_with(cx, |view, _| view.doc.root_scroll_offset())?;
    if at != Some(offset) {
        return Err(format!(
            "the fresh document opened at {at:?}, not at the scroll offset {offset:?}"
        )
        .into());
    }
    let mut check = super::fresh_check::compare(
        &incremental,
        &fresh.pixels,
        args.fresh_check_dir.as_deref(),
        "scroll",
    )?;
    check["root_scroll_offset"] = json!([offset.0, offset.1]);
    let different = check["different_rgb_pixels"].as_u64().unwrap_or(u64::MAX);
    let mode = &mut report["modes"]["scroll"];
    mode["matches_fresh"] = json!(different == 0);
    mode["fresh_checks"] = json!([check]);
    if different != 0 {
        return Err(format!(
            "the scroll frame at {offset:?} differs from a fresh document at that offset in {different} RGB pixels"
        )
        .into());
    }
    Ok(())
}
