//! Режим `edit`: кадры после локальной правки разметки (`edit::Edit`) через
//! `Document::update`, по очереди правка и возврат, затем проверка, что каждый
//! вариант совпадает по пикселям со свежим документом той же разметки.

use super::{
    args::Args,
    history::History,
    pixels, resources,
    run::{Step, sample},
    view::View,
};
use gpui::{AsyncApp, WindowHandle};
use kamin_html::host::capture::CaptureSession;
use kamin_html::metrics::FontContext;
use serde_json::json;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) async fn run(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    fonts: &FontContext,
    args: &Args,
    capture: &CaptureSession,
    luid: (i32, u32),
) -> Result<History> {
    let base = resources::read(&args.path)?;
    let edited = args.edit.markup(&base)?;
    // Вариант 0 — вход, 1 — правка; первый замер идёт после холодного кадра входа.
    let variants = [base.as_str(), edited.as_str()];
    let mut history = History::new()?;
    let mut last = [None, None];
    for index in 1..=args.runs {
        let variant = index % 2;
        let (measured, frame, (work, resources)) = sample(
            cx,
            window,
            fonts,
            args,
            Step::Update(variants[variant]),
            capture,
            luid,
        )
        .await?;
        history.push(measured, work, resources)?;
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
        let different = pixels::differing(incremental, &fresh.pixels)?;
        let mut check = json!({"variant": variant, "different_rgb_pixels": different});
        if different != 0 {
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
    let mode = history.metadata();
    mode["edit"] = json!(format!("{:?}", args.edit).to_lowercase());
    mode["matches_fresh"] = json!(checks.iter().all(|c| c["different_rgb_pixels"] == 0));
    mode["fresh_checks"] = json!(checks);
    Ok(history)
}
