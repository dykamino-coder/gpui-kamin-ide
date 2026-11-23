//! Optional edit/scroll RGB acceptance outside the measured series.
//! ENGINE_BENCH_EDIT_SCROLL_CHECK sets a positive scroll step in CSS pixels.
//! The final viewport must reveal the effect of the edit.

use super::{
    args::{Args, Mode},
    frames::Run,
    run::{Step, sample},
};
use gpui::frame_capture::CapturedFrame;
use serde_json::{Value, json};
use std::rc::Rc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn enabled(args: &Args) -> bool {
    args.mode == Mode::Edit && std::env::var_os("ENGINE_BENCH_EDIT_SCROLL_CHECK").is_some()
}

/// Each side opens independently so the reference cannot inherit dirty state.
async fn fresh(html: &str, offset: (f32, f32), run: Run<'_, '_>) -> Result<Rc<CapturedFrame>> {
    let (cx, window, fonts, args, capture, luid) = run;
    sample(
        cx,
        window,
        fonts,
        args,
        Step::Cold(Some(html)),
        capture,
        luid,
    )
    .await?;
    super::memory::before_phase(cx, capture, args).await?;
    window.update(cx, |view, _, _| view.doc.set_root_scroll_offset(offset))?;
    sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
    super::memory::before_phase(cx, capture, args).await?;
    let (_, frame, _) = sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
    let at = window.read_with(cx, |view, _| view.doc.root_scroll_offset())?;
    if at != Some(offset) {
        return Err(format!("edit-scroll reference opened at {at:?}, expected {offset:?}").into());
    }
    Ok(frame.pixels)
}

pub(super) async fn check(base: &str, edited: &str, run: Run<'_, '_>) -> Result<Option<Value>> {
    let (cx, window, fonts, args, capture, luid) = run;
    if !enabled(args) {
        return Ok(None);
    }
    let distance: f32 = std::env::var("ENGINE_BENCH_EDIT_SCROLL_CHECK")?.parse()?;
    if !distance.is_finite() || distance <= 0. {
        return Err("ENGINE_BENCH_EDIT_SCROLL_CHECK must be finite and positive".into());
    }
    super::animation::prepare(cx, window, args, 0)?;
    sample(
        cx,
        window,
        fonts,
        args,
        Step::Cold(Some(base)),
        capture,
        luid,
    )
    .await?;
    super::memory::before_phase(cx, capture, args).await?;
    let before = window.read_with(cx, |view, _| view.doc.frame_counters())?;
    let (_, edit, _) = sample(cx, window, fonts, args, Step::Edit(edited), capture, luid).await?;
    let after = window.read_with(cx, |view, _| view.doc.frame_counters())?;
    let (built, replayed, _) = after
        .since(&before)
        .ok_or("edit-scroll counters regressed")?;
    window.update(cx, |view, _, _| {
        view.doc.set_root_scroll_offset((0., distance))
    })?;
    let (_, scrolled, _) = sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
    let offset = window
        .read_with(cx, |view, _| view.doc.root_scroll_offset())?
        .ok_or("edit-scroll check has no root scroller")?;
    if offset.1 <= 0. {
        return Err("edit-scroll fixture did not scroll".into());
    }
    super::memory::before_phase(cx, capture, args).await?;
    let (_, settled, _) = sample(cx, window, fonts, args, Step::Warm, capture, luid).await?;
    let control = fresh(edited, offset, (cx, window, fonts, args, capture, luid)).await?;
    let old = fresh(base, offset, (cx, window, fonts, args, capture, luid)).await?;
    let negative = super::pixels::differing(&old, &control)?;
    if negative == 0 {
        return Err("edit-scroll fixture never showed the edited pixels".into());
    }
    let at_top = fresh(edited, (0., 0.), (cx, window, fonts, args, capture, luid)).await?;
    let mut checks = Vec::new();
    for (name, actual, expected) in [
        ("edit-scroll-edit", &edit.pixels, &at_top),
        ("edit-scroll-first", &scrolled.pixels, &control),
        ("edit-scroll-settled", &settled.pixels, &control),
    ] {
        let mut check =
            super::fresh_check::compare(actual, expected, args.fresh_check_dir.as_deref(), name)?;
        check["phase"] = json!(name);
        checks.push(check);
    }
    let matched = super::fresh_check::matches_series(3, &checks);
    if !matched {
        return Err(format!("edit-scroll RGB mismatch: {checks:?}").into());
    }
    Ok(Some(json!({
        "matches_fresh": matched, "checks": checks, "root_scroll_offset": offset,
        "negative_control_different_rgb_pixels": negative,
        "edit_built_frames": built,
        "edit_replayed_frames": replayed,
    })))
}
