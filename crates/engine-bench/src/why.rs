//! `--why DIR`: the first reader of the DevTools `why` journal
//! (`kamin_html::why`). Each sampled frame gets one entry: built or replayed,
//! the journal's causes with their engine lines, the requests of frames and
//! the draws and renders they caused, the paint-path refusals, the phase
//! times, the cache lookups with the reasons of their misses and the memory
//! by owner with what grew. A failed sample keeps its entry with the error.
//! `why.json` (schema `kamin_html::why::SCHEMA`) is for the browser panel
//! and `tools/engine-compare`; `why.txt` for people; `--why-compare A B`
//! reads two of them (`compare`).

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
                    let report = report.get_or_insert_with(kamin_html::why::Report::new);
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
        measurements: Option<(&super::report::Sample, &super::work::Sample)>,
        raster: Option<gpui::frame_capture::RasterReport>,
        error: Option<String>,
    ) -> Result<()> {
        let (phases, work) = measurements.unzip();
        let private = super::process::Snapshot::read()?.commit;
        let gpu_batches = cx.update(|cx| cx.instrumentation().batches.take_line());
        let cold = matches!(step, Step::Cold(_));
        let fields = json!({
            "mode": format!("{:?}", args.mode).to_lowercase(),
            "step": match step { Step::Cold(_) => "cold", Step::Warm => "warm", Step::Edit(_) => "edit" },
            "error": error, "phases_ms": phases,
            "paint_work": work.map(super::work::Sample::paint_profile),
            "raster": raster.map(kamin_html::why::raster),
            "gpu_batches": gpu_batches,
            "memory": {"process_private_bytes": private},
        });
        window.update(cx, |view, window, cx| {
            REPORT.with(|report| {
                let mut report = report.borrow_mut();
                report
                    .as_mut()
                    .expect("sample began a report")
                    .record(self.0, &view.doc, window, cx, cold, fields)
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
