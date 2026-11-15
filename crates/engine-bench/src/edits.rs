//! Режим `edit`: кадры после локальной правки разметки (`edit::Edit`),
//! применённой записями мутаций DOM (`Document::apply`), а когда правка меняет
//! форму дерева (вставка, удаление узла) — `Document::update`, по очереди правка и
//! возврат, затем проверка, что каждый вариант совпадает по пикселям со свежим
//! документом той же разметки. Отчёт режима несёт работу документа каждого
//! кадра правки (`document_frame_samples`: сборки, повторы, сборки внутри draw).

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
    let mut frame_samples = Vec::with_capacity(args.runs);
    for index in 1..=args.runs {
        let variant = index % 2;
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
        // Each edit commits a new revision whose frame slot counts from zero:
        // its counters after the frame are the work of this frame.
        frame_samples.push(window.read_with(cx, counters)?);
        samples.push(measured.phases);
        diagnostics.push(measured.diagnostics);
        work.push(counts);
        last[variant] = Some(frame.pixels);
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
    mode["matches_fresh"] = json!(checks.iter().all(|c| c["different_rgb_pixels"] == 0));
    mode["fresh_checks"] = json!(checks);
    mode["document_frame_samples"] = frame_samples
        .iter()
        .enumerate()
        .map(|(i, &counts)| frame_sample(i + 1, counts))
        .collect::<Result<Value>>()?;
    Ok((mode, work))
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
