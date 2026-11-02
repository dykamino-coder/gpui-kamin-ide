//! Режим `edit`: кадры после локальной правки разметки (`edit::Edit`) через
//! `Document::update`, по очереди правка и возврат, затем проверка, что каждый
//! вариант совпадает по пикселям со свежим документом той же разметки.

use super::{
    args::Args,
    pixels, report, resources,
    run::{Step, sample},
    view::View,
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
) -> Result<Value> {
    let base = resources::read(&args.path)?;
    let edited = args.edit.markup(&base)?;
    // Вариант 0 — вход, 1 — правка; первый замер идёт после холодного кадра входа.
    let variants = [base.as_str(), edited.as_str()];
    let mut samples = Vec::with_capacity(args.runs);
    let mut diagnostics = Vec::with_capacity(args.runs);
    let mut last = [None, None];
    for index in 1..=args.runs {
        let variant = index % 2;
        let (measured, frame, _) = sample(
            cx,
            window,
            fonts,
            args,
            Step::Update(variants[variant]),
            capture,
            luid,
        )
        .await?;
        samples.push(measured.phases);
        diagnostics.push(measured.diagnostics);
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
        checks.push(json!({"variant": variant,
            "different_rgb_pixels": pixels::differing(incremental, &fresh.pixels)?}));
    }
    let mut mode = report::mode(&samples);
    mode["diagnostic_scopes"] = report::mode(&diagnostics);
    mode["edit"] = json!(format!("{:?}", args.edit).to_lowercase());
    mode["matches_fresh"] = json!(checks.iter().all(|c| c["different_rgb_pixels"] == 0));
    mode["fresh_checks"] = json!(checks);
    Ok(mode)
}
