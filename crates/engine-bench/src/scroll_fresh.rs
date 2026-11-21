//! Scroll mode: every wheel-step frame (built or replayed scrolled) must
//! equal a fresh document opened at the same root scroll offset. A replay
//! that draws stale content (a stationary layer that should scroll, a moved
//! copy without its deferred draws) fails the run instead of only timing it.
//! Fresh documents open after all measured steps, so the checks never touch
//! the timed frames.

use super::frames::Run;
use super::run::{Step, sample};
use super::step_frames::StepFrames;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) async fn check(steps: StepFrames, report: &mut Value, run: Run<'_, '_>) -> Result<()> {
    let (cx, window, fonts, args, capture, luid) = run;
    let steps = steps.steps;
    if steps.is_empty() {
        return Err("scroll mode captured no frame to check against a fresh document".into());
    }
    let mut checks = Vec::with_capacity(steps.len());
    let mut failures = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        super::animation::prepare(cx, window, args, index + 1)?;
        let offset = step.offset;
        window.update(cx, |view, _, _| view.fresh_scroll = Some(offset))?;
        sample(cx, window, fonts, args, Step::Cold(None), capture, luid).await?;
        // The open's first frame lays the root out before it knows its own
        // scrollbars and draws again: the reference is the frame the open
        // settles on, as scroll mode settles the load before its steps.
        super::memory::converge(cx, capture, 5).await?;
        let before = window.read_with(cx, |view, _| view.doc.retained_frames())?;
        let (_, fresh, _) = sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
        let after = window.read_with(cx, |view, _| view.doc.retained_frames())?;
        if after.0 != before.0 {
            return Err(
                format!("the fresh document at {offset:?} still builds after it settled").into(),
            );
        }
        let at = window.read_with(cx, |view, _| view.doc.root_scroll_offset())?;
        if at != Some(offset) {
            return Err(format!(
                "the fresh document opened at {at:?}, not at the scroll offset {offset:?}"
            )
            .into());
        }
        let name = format!("scroll-{}", index + 1);
        let dir = args.fresh_check_dir.as_deref();
        let mut check = super::fresh_check::compare(&step.measured, &fresh.pixels, dir, &name)?;
        check["root_scroll_offset"] = json!([offset.0, offset.1]);
        if let Some(settled) = &step.settled {
            let name = format!("{name}-settled");
            let settled = super::fresh_check::compare(settled, &fresh.pixels, dir, &name)?;
            check["settled_different_rgb_pixels"] = settled["different_rgb_pixels"].clone();
        }
        let different = check["different_rgb_pixels"].as_u64().unwrap_or(u64::MAX);
        if different != 0 {
            failures.push(format!("step {} at {offset:?}: {different}", index + 1));
        }
        checks.push(check);
    }
    let mode = &mut report["modes"]["scroll"];
    mode["matches_fresh"] = json!(failures.is_empty());
    mode["fresh_checks"] = json!(checks);
    if !failures.is_empty() {
        return Err(format!(
            "scroll frames differ from a fresh document at the same offset (RGB pixels): {}",
            failures.join(", ")
        )
        .into());
    }
    Ok(())
}
