//! Непересекающийся бюджет кадра; вложенные счётчики вынесены в диагностику.

use super::{
    report::{Sample, ms},
    view::Phases,
};
use gpui::frame_perf;
use kamin_html::BuildTimings;
use kamin_html::host::capture::CaptureSession;
use std::{collections::BTreeMap, sync::atomic::Ordering::Relaxed, time::Duration};

pub(super) struct Counters {
    pub(super) draw: u32,
    pub(super) present: u32,
    pub(super) layout: u32,
    pub(super) frames: u32,
    pub(super) readback: Duration,
    pub(super) copy: Duration,
    pub(super) gpu_wait: Duration,
}

impl Counters {
    pub(super) fn read(capture: &CaptureSession) -> Self {
        Self {
            draw: frame_perf::DRAW_US.load(Relaxed),
            present: frame_perf::PRESENT_US.load(Relaxed),
            layout: frame_perf::TAFFY_US.load(Relaxed),
            frames: frame_perf::FRAMES.load(Relaxed),
            readback: capture.readback_time(),
            copy: capture.copy_time(),
            gpu_wait: capture.gpu_wait_time(),
        }
    }
    pub(super) fn delta(current: u32, previous: u32) -> Duration {
        Duration::from_micros(u64::from(current.wrapping_sub(previous)))
    }
}

pub(super) struct Measured {
    pub(super) phases: Sample,
    pub(super) diagnostics: Sample,
}

pub(super) struct Input {
    pub(super) resources: Option<Duration>,
    pub(super) document: Option<Duration>,
    pub(super) timings: Option<BuildTimings>,
    pub(super) layout: Duration,
    pub(super) draw: Duration,
    pub(super) present: Duration,
    pub(super) readback: Duration,
    /// Часть readback после готовности кадра на GPU (копия в память): цена
    /// стенда, которой нет у браузеров; доска вычитает её из кадра с растром.
    pub(super) copy: Duration,
    /// The readback's wait for the GPU to execute the frame (fence). The window
    /// path never waits for it on a hardware GPU; on WARP it is the CPU raster.
    pub(super) gpu_wait: Duration,
    pub(super) total: Duration,
    pub(super) readback_to_resume: Duration,
}

fn subtract(parent: Duration, child: Duration) -> Result<Duration, &'static str> {
    parent
        .checked_sub(child)
        .ok_or("nested duration exceeds its parent; ambiguous phase accounting")
}

pub(super) fn partition(input: Input, phases: &Phases) -> Result<Measured, &'static str> {
    let mut intervals = BTreeMap::new();
    intervals.insert("host_resources", input.resources);
    intervals.insert("parse", input.timings.map(|t| t.parse));
    intervals.insert("style", input.timings.map(|t| t.style));
    let document_other = match (input.document, input.timings) {
        (Some(document), Some(t)) => Some(subtract(subtract(document, t.parse)?, t.style)?),
        (None, None) => None,
        _ => return Err("document timing and parse/style availability disagree"),
    };
    intervals.insert("document_other", document_other);
    let mut diagnostics = Sample::from([
        ("observer_setup", Some(ms(phases.observer_setup))),
        ("request_to_render", phases.request_to_render.map(ms)),
        ("document_build_total", input.document.map(ms)),
        ("cpu_draw_total", Some(ms(input.draw))),
        ("gpu_readback", Some(ms(input.readback))),
        ("capture_copy", Some(ms(input.copy))),
        ("taffy_inclusive", Some(ms(input.layout))),
        ("readback_to_resume", Some(ms(input.readback_to_resume))),
        // A count, not a time: renders of a layout retry inside the draw,
        // whose time the phases above already hold.
        ("layout_retries", Some(phases.layout_retries as f64)),
    ]);
    for (name, scope) in [
        ("build_elements", &phases.build),
        ("layout_request", &phases.request_layout),
        ("prepaint", &phases.prepaint),
        ("paint", &phases.paint),
    ] {
        intervals.insert(name, Some(*scope));
    }
    // raster_gpu_readback = raster_submit + gpu_wait + capture, exactly.
    let submit = subtract(input.present, input.readback)?;
    let capture = subtract(input.readback, input.gpu_wait)?;
    subtract(capture, input.copy)?;
    diagnostics.insert("raster_submit", Some(ms(submit)));
    diagnostics.insert("gpu_wait", Some(ms(input.gpu_wait)));
    diagnostics.insert("capture", Some(ms(capture)));
    intervals.insert(
        "layout",
        Some(
            phases
                .layout
                .ok_or("missing root request/prepaint boundary")?,
        ),
    );
    intervals.insert("raster_gpu_readback", Some(input.present));
    let measured: Duration = intervals.values().flatten().copied().sum();
    intervals.insert("pipeline_other", Some(subtract(input.total, measured)?));
    intervals.insert("first_frame_total", Some(input.total));
    Ok(Measured {
        phases: intervals.into_iter().map(|(k, v)| (k, v.map(ms))).collect(),
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us(n: u64) -> Duration {
        Duration::from_micros(n)
    }

    fn input() -> Input {
        Input {
            resources: Some(us(2)),
            document: Some(us(10)),
            timings: Some(BuildTimings {
                parse: us(3),
                style: us(5),
            }),
            layout: us(100),
            draw: us(30),
            present: us(10),
            readback: us(4),
            copy: us(3),
            gpu_wait: us(1),
            total: us(60),
            readback_to_resume: us(1),
        }
    }

    #[test]
    fn nested_layout_is_diagnostic_and_document_scopes_partition_the_complete_frame() {
        let mut phases = Phases::default();
        phases.layout = Some(us(6));
        phases.prepaint = us(20);
        let result = partition(input(), &phases).expect("valid nesting");
        assert_eq!(result.phases["prepaint"], Some(0.020));
        assert_eq!(result.phases["document_other"], Some(0.002));
        let sum: f64 = result
            .phases
            .iter()
            .filter(|(k, _)| **k != "first_frame_total")
            .filter_map(|(_, v)| *v)
            .sum();
        assert!((sum - result.phases["first_frame_total"].expect("total")).abs() < 1e-15);
        assert_eq!(result.diagnostics["taffy_inclusive"], Some(0.100));
        assert_eq!(result.diagnostics["gpu_readback"], Some(0.004));
        assert_eq!(result.diagnostics["capture_copy"], Some(0.003));
        assert_eq!(result.diagnostics["raster_submit"], Some(0.006));
        assert_eq!(result.diagnostics["gpu_wait"], Some(0.001));
        assert_eq!(result.diagnostics["capture"], Some(0.003));
    }

    #[test]
    fn a_gpu_wait_longer_than_the_readback_is_an_error() {
        let mut phases = Phases::default();
        phases.layout = Some(us(6));
        let mut wait = input();
        wait.gpu_wait = us(2);
        assert!(
            partition(wait, &phases).is_err(),
            "wait + copy exceed the readback"
        );
    }

    #[test]
    fn inconsistent_nesting_is_an_error_instead_of_clamping_or_dropping_samples() {
        assert!(partition(input(), &Phases::default()).is_err());
        let mut phases = Phases::default();
        phases.layout = Some(us(6));
        let mut short = input();
        short.total = us(1);
        assert!(partition(short, &phases).is_err());
        let mut document = input();
        document.document = Some(us(4));
        assert!(partition(document, &phases).is_err());
    }

    #[test]
    fn a_layout_retry_is_one_frame_and_its_count_is_recorded() {
        let mut phases = Phases::default();
        phases.layout = Some(us(6));
        phases.renders = 3;
        phases.layout_retries = 2;
        assert_eq!(phases.frame_renders(), 1);
        let result = partition(input(), &phases).expect("one draw");
        assert_eq!(result.diagnostics["layout_retries"], Some(2.0));
        let total = result.phases["first_frame_total"].expect("total");
        assert!((total - 0.060).abs() < 1e-12, "the whole draw: {total}");
    }

    #[test]
    fn warm_frame_has_no_document_work() {
        let mut input = input();
        input.resources = None;
        input.document = None;
        input.timings = None;
        let mut phases = Phases::default();
        phases.layout = Some(us(6));
        let result = partition(input, &phases).expect("warm frame");
        assert_eq!(result.phases["parse"], None);
        assert_eq!(result.phases["document_other"], None);
    }
}
