//! Alternating edits with exact RGB validation of every captured transition.
//! Missing captures and intermediate mismatches cannot produce a passing verdict.

use super::{
    args::Args,
    report, resources,
    run::{Step, sample},
    view::View,
    work,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::CaptureSession;
use kamin_html::metrics::FontContext;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: &super::work::Adapter,
) -> Result<(Value, Vec<work::Sample>)> {
    let base = resources::read(&args.path)?;
    let edited = match args.edit_markup.as_deref() {
        Some(path) => resources::read(path)?,
        None => args.edit.markup(&base)?,
    };
    // Вариант 0 — вход, 1 — правка; первый замер идёт после холодного кадра входа.
    let variants = [base.as_str(), edited.as_str()];
    // Документ сейчас держит вход: форма правки видна до первого замера.
    let same_shape = window.read_with(cx, |view, _| view.doc.mutations(&edited).is_some())?;
    // Boxes of the input that paint only their own commands, and refusals by reason.
    let census = window.read_with(cx, |view, _| kamin_html::paint_path::census(&view.doc))?;
    let quiet = super::memory::before_phase(cx, capture, args).await?;
    let mut samples = Vec::with_capacity(args.runs);
    let mut diagnostics = Vec::with_capacity(args.runs);
    let mut work = Vec::with_capacity(args.runs);
    // Fresh Documents do not inherit incremental layout/style state. Keep only
    // two references, then restore the input before the measured transitions.
    let mut references = Vec::with_capacity(2);
    for markup in variants {
        let (_, fresh, _) = sample(
            cx,
            window,
            fonts,
            args,
            Step::Cold(Some(markup)),
            capture,
            luid,
        )
        .await?;
        references.push(fresh.pixels);
    }
    sample(
        cx,
        window,
        fonts,
        args,
        Step::Cold(Some(variants[0])),
        capture,
        luid,
    )
    .await?;
    let mut checks = Vec::with_capacity(args.runs);
    let mut skipped_checks = 0;
    let mut layer = Vec::with_capacity(args.runs);
    // ENGINE_BENCH_PRESENT_SKIP: the edits may skip presentation (a change
    // off screen); such samples end where the frame decided it.
    let present = std::env::var_os("ENGINE_BENCH_PRESENT_SKIP").is_none();
    // The bench asks for every frame it reads (`require_presentation`).
    window.update(cx, |_, window, _| window.allow_unpresented_readback(true))?;
    let mut frame_samples = Vec::with_capacity(args.runs);
    let series = super::series::Series::new(args);
    let series_start = std::time::Instant::now();
    let mut index = 0;
    while series.more(index, series_start.elapsed()) {
        index += 1;
        let variant = index % 2;
        let path_before = kamin_html::paint_path::read();
        let before = (
            layer_work(),
            window.read_with(cx, |view, _| view.doc.retained_frames())?,
        );
        let (measured, frame, counts) = super::run::sample_with(
            cx,
            window,
            fonts,
            args,
            Step::Edit(variants[variant]),
            capture,
            luid,
            present,
        )
        .await?;
        // Each edit commits a new revision whose frame slot counts from zero:
        // its counters after the frame are the work of this frame.
        frame_samples.push(window.read_with(cx, counters)?);
        samples.push(measured.phases);
        diagnostics.push(measured.diagnostics);
        work.push(counts);
        let after = (
            layer_work(),
            window.read_with(cx, |view, _| view.doc.retained_frames())?,
        );
        let mut delta = layer_delta(before, after);
        let path_after = kamin_html::paint_path::read();
        for (i, name) in kamin_html::paint_path::NAMES.iter().enumerate() {
            delta[format!("path_{name}")] = json!(path_after[i].wrapping_sub(path_before[i]));
        }
        layer.push(delta);
        // Compare the actual measured frame after its timing and counters close.
        // A skipped presentation has no captured pixels and cannot prove equality.
        if let Some(frame) = frame {
            let mut check = super::fresh_check::compare(
                &frame.pixels,
                &references[variant],
                args.fresh_check_dir.as_deref(),
                &format!("edit-{index}-variant-{variant}"),
            )?;
            check["sample_index"] = json!(index);
            check["variant"] = json!(variant);
            if check["different_rgb_pixels"] != 0
                && let Some(directory) = &args.frame_diff_dir
            {
                super::fresh_check::compare(
                    &frame.pixels,
                    &references[variant],
                    Some(directory),
                    &format!("edit-{index}-variant-{variant}"),
                )?;
            }
            checks.push(check);
        } else {
            skipped_checks += 1;
        }
    }
    let mut mode = report::measured_mode(&samples, &diagnostics);
    mode["edit"] = match args.edit_markup.as_deref() {
        Some(path) => json!({"markup": path}),
        None => json!(format!("{:?}", args.edit).to_lowercase()),
    };
    mode["edit_input"] = json!(if same_shape {
        "mutation_records"
    } else {
        "document_update"
    });
    mode["presentation_required"] = json!(present);
    mode["scene_layer"] = json!({"samples": layer, "census": census});
    // A timed series knows its length only now: every measured frame must be checked.
    let required = samples.len();
    mode["matches_fresh"] = json!(super::fresh_check::matches_series(required, &checks));
    mode["fresh_validation"] = json!({
        "required": required, "compared": checks.len(), "skipped": skipped_checks,
        "status": if checks.is_empty() { "not_checked" }
            else if checks.len() != required { "incomplete" }
            else if super::fresh_check::matches_series(required, &checks) { "matched" } else { "mismatched" },
        "method": "each_measured_capture_against_fresh_document",
    });
    mode["fresh_checks"] = json!(checks);
    mode["start_quiet"] = super::memory::quiet_summary(&quiet);
    mode["document_frame_samples"] = frame_samples
        .iter()
        .enumerate()
        .map(|(i, &counts)| frame_sample(i + 1, counts))
        .collect::<Result<Value>>()?;
    Ok((mode, work))
}

/// The scene layer's process-wide work counters (`gpui::layer_counters`).
fn layer_work() -> [u64; 6] {
    use gpui::layer_counters as c;
    use std::sync::atomic::Ordering::Relaxed;
    [
        c::RECORDS.load(Relaxed),
        c::COW_CLONES.load(Relaxed),
        c::VALUES_WRITTEN.load(Relaxed),
        c::BOXES_RERECORDED.load(Relaxed),
        c::PRIMITIVES_COPIED.load(Relaxed),
        c::PRESENTS_SKIPPED.load(Relaxed),
    ]
}

/// One sample's layer work and frame builds/replays of the document's
/// retained frame (a rebuilt document counts from zero again).
fn layer_delta(before: ([u64; 6], (u64, u64)), after: ([u64; 6], (u64, u64))) -> Value {
    let d = |i: usize| after.0[i].wrapping_sub(before.0[i]);
    let since = |a: u64, b: u64| a.checked_sub(b).unwrap_or(a);
    json!({
        "layers_recorded": d(0),
        "cow_clones": d(1),
        "values_written": d(2),
        "boxes_rerecorded": d(3),
        "primitives_copied": d(4),
        "presents_skipped": d(5),
        "frame_builds": since(after.1.0, before.1.0),
        "frame_replays": since(after.1.1, before.1.1),
    })
}

/// Сборки, повторы и сборки внутри `Window::draw` документа на эту минуту.
fn counters(view: &View, _: &gpui::App) -> (u64, u64, u64) {
    let (builds, replays) = view.doc.retained_frames();
    (builds, replays, view.doc.builds_inside_draw())
}

/// Работа документа за кадр правки; кадр без сборки и без повтора значит, что
/// счётчики не начались с ревизии правки, и не выдаётся за ноль работы.
fn frame_sample(index: usize, (builds, replays, inside): (u64, u64, u64)) -> Result<Value> {
    if builds + replays == 0 {
        return Err(format!("edit frame {index} neither built nor replayed the document").into());
    }
    Ok(
        json!({"mode": "edit", "sample_index": index, "builds": builds, "replays": replays,
        "builds_inside_draw": inside}),
    )
}
