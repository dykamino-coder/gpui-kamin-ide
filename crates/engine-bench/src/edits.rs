//! Режим `edit`: кадры после локальной правки разметки (`edit::Edit`),
//! применённой записями мутаций DOM (`Document::apply`), а когда правка меняет
//! форму дерева (вставка, удаление узла) — `Document::update`, по очереди правка и
//! возврат, затем проверка, что каждый вариант совпадает по пикселям со свежим
//! документом той же разметки.

use super::{
    args::Args,
    pixels, report, resources,
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
    luid: (i32, u32),
) -> Result<(Value, Vec<work::Sample>)> {
    let base = resources::read(&args.path)?;
    let edited = match super::edit::markup_file() {
        Some(path) => resources::read(path)?,
        None => args.edit.markup(&base)?,
    };
    // Вариант 0 — вход, 1 — правка; первый замер идёт после холодного кадра входа.
    let variants = [base.as_str(), edited.as_str()];
    // Документ сейчас держит вход: форма правки видна до первого замера.
    let same_shape = window.read_with(cx, |view, _| view.doc.mutations(&edited).is_some())?;
    super::memory::steady(cx, capture).await?;
    let mut samples = Vec::with_capacity(args.runs);
    let mut diagnostics = Vec::with_capacity(args.runs);
    let mut work = Vec::with_capacity(args.runs);
    let mut last = [None, None];
    let mut layer = Vec::with_capacity(args.runs);
    for index in 1..=args.runs {
        let variant = index % 2;
        let before = (layer_work(), window.read_with(cx, |view, _| view.doc.retained_frames())?);
        let (measured, frame, counts) = sample(
            cx,
            window,
            fonts,
            args,
            Step::Edit(variants[variant]),
            capture,
            luid,
        )
        .await?;
        samples.push(measured.phases);
        diagnostics.push(measured.diagnostics);
        work.push(counts);
        last[variant] = Some(frame.pixels);
        let after = (layer_work(), window.read_with(cx, |view, _| view.doc.retained_frames())?);
        layer.push(layer_delta(before, after));
    }
    let mut checks = Vec::new();
    for (variant, incremental) in last.iter().enumerate() {
        let Some(incremental) = incremental else {
            continue;
        };
        let markup = variants[variant];
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
        let mut check = super::fresh_check::compare(
            incremental,
            &fresh.pixels,
            args.fresh_check_dir.as_deref(),
            &format!("edit-{variant}"),
        )?;
        check["variant"] = json!(variant);
        if check["different_rgb_pixels"] != 0 {
            // Второй свежий кадр отличает недетерминизм свежей сборки от
            // ошибки инкрементального пути.
            let (_, again, _) = sample(
                cx,
                window,
                fonts,
                args,
                Step::Cold(Some(markup)),
                capture,
                luid,
            )
            .await?;
            check["fresh_vs_fresh_rgb_pixels"] =
                json!(pixels::differing(&fresh.pixels, &again.pixels)?);
            check["incremental_vs_second_fresh_rgb_pixels"] =
                json!(pixels::differing(incremental, &again.pixels)?);
            check["second_fresh"] = super::fresh_check::compare(
                incremental,
                &again.pixels,
                args.fresh_check_dir.as_deref(),
                &format!("edit-{variant}-second"),
            )?;
            check["detail"] = pixels::detail(incremental, &fresh.pixels);
            if let Some(dir) = &args.frame_diff_dir {
                std::fs::create_dir_all(dir)?;
                pixels::save(
                    incremental,
                    &dir.join(format!("edit-{variant}-incremental.png")),
                )?;
                pixels::save(
                    &fresh.pixels,
                    &dir.join(format!("edit-{variant}-fresh.png")),
                )?;
            }
        }
        checks.push(check);
    }
    let mut mode = report::measured_mode(&samples, &diagnostics);
    mode["edit"] = match super::edit::markup_file() {
        Some(path) => json!({"markup": path}),
        None => json!(format!("{:?}", args.edit).to_lowercase()),
    };
    mode["edit_input"] = json!(if same_shape {
        "mutation_records"
    } else {
        "document_update"
    });
    mode["scene_layer"] = json!({"samples": layer});
    mode["matches_fresh"] = json!(checks.iter().all(|c| c["different_rgb_pixels"] == 0));
    mode["fresh_checks"] = json!(checks);
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