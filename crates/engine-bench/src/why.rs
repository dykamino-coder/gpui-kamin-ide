//! `--why DIR`: the first reader of the DevTools `why` journal
//! (`kamin_html::why`). Each sampled frame gets one entry: built or replayed,
//! the journal's causes with their engine lines, the requests of frames and
//! the draws and renders they caused, the paint-path refusals, the phase
//! times and the cache lookups with the reasons of their misses. Explicit
//! memory snapshots show owners and what grew. A failed sample keeps its error.
//! `why.json` (schema `kamin_html::why::SCHEMA`) is for the browser panel
//! and `tools/engine-compare`; `why.txt` for people; `--why-compare A B`
//! reads two of them (`compare`). Detailed text geometry is opt-in with
//! `KAMIN_DIAG_TEXT_GEOMETRY=1`, so ordinary journal timing stays comparable.

pub(super) mod compare;

use super::run::Step;
use super::view::View;
use gpui::{AsyncApp, WindowHandle};
use serde_json::json;
use std::cell::RefCell;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

thread_local! {
    static REPORT: RefCell<Option<kamin_html::why::Report>> = const { RefCell::new(None) };
}

/// One sample in the shared engine journal.
pub(super) struct Watch(kamin_html::why::Watch);

impl Watch {
    pub(super) fn begin(
        args: &super::args::Args,
        cx: &AsyncApp,
        window: WindowHandle<View>,
    ) -> Result<Option<Self>> {
        if args.why.is_none() {
            return Ok(None);
        }
        window
            .read_with(cx, |view, _| {
                REPORT.with(|report| {
                    let mut report = report.borrow_mut();
                    let report = report.get_or_insert_with(|| {
                        if std::env::var_os("KAMIN_DIAG_TEXT_GEOMETRY").is_some_and(|v| v == "1") {
                            kamin_html::why::Report::with_text_geometry()
                        } else {
                            kamin_html::why::Report::new()
                        }
                    });
                    Some(Self(report.watch(&view.doc)))
                })
            })
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn end(
        self,
        args: &super::args::Args,
        cx: &mut AsyncApp,
        window: WindowHandle<View>,
        step: Step<'_>,
        measurements: Option<(&super::accounting::Measured, &super::work::Sample)>,
        raster: Option<gpui::frame_capture::RasterReport>,
        error: Option<String>,
    ) -> Result<()> {
        let (measured, work) = measurements.unzip();
        let phases = measured.map(|m| &m.phases);
        let part = |key: &str| measured.and_then(|m| m.diagnostics.get(key).copied().flatten());
        let private = super::process::Snapshot::read()?.commit;
        let gpu_batches = cx.update(|cx| cx.instrumentation().batches.take_line());
        let cold = matches!(step, Step::Cold(_));
        let kinds = cx.update(|cx| {
            kamin_html::why::request_layout_kinds(cx.instrumentation().request_layout.take())
        });
        let raster = raster.map(|r| {
            let mut fields = kamin_html::why::raster(r);
            fields["submit_ms"] = json!(part("raster_submit"));
            fields["capture_copy_ms"] = json!(part("capture_copy"));
            fields
        });
        let inputs = cx
            .update(|cx| kamin_html::why::layout_inputs(cx.instrumentation().layout_inputs.take()));
        let fields = json!({
            "mode": format!("{:?}", args.mode).to_lowercase(),
            "step": match step { Step::Cold(_) => "cold", Step::Warm => "warm", Step::Edit(_) => "edit" },
            "error": error, "phases_ms": phases,
            "paint_work": work.map(super::work::Sample::paint_profile),
            "layout_work": work.map(super::work::Sample::layout_profile),
            "raster": raster,
            "request_layout_kinds": kinds,
            "layout_inputs": inputs,
            "gpu_batches": gpu_batches,
            "memory": {"process_private_bytes": private},
        });
        window.update(cx, |view, window, cx| {
            REPORT.with(|report| {
                let mut report = report.borrow_mut();
                let report = report.as_mut().expect("sample began a report");
                if cold {
                    report.sample_memory("open", &view.doc, window, cx, Some(private));
                }
                report.record(self.0, &view.doc, cold, fields)
            })
        })??;
        REPORT.with(|report| {
            report
                .borrow()
                .as_ref()
                .expect("sample report")
                .write(args.why.as_deref().ok_or("--why without a directory")?)
        })
    }
}

pub(super) fn finish(
    args: &super::args::Args,
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
) -> Result<()> {
    if args.why.is_none() {
        return Ok(());
    }
    let private = super::process::Snapshot::read()?.commit;
    window.update(cx, |view, window, cx| {
        REPORT.with(|report| {
            if let Some(report) = report.borrow_mut().as_mut() {
                report.sample_memory("series_end", &view.doc, window, cx, Some(private));
                report.write(args.why.as_deref().expect("enabled journal"))?;
            }
            Ok(())
        })
    })?
}
