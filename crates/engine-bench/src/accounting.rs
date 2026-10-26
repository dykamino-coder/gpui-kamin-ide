//! Непересекающийся бюджет кадра; вложенные счётчики вынесены в диагностику.

use super::{
    report::{Sample, ms},
    view::Phases,
};
use kamin_html::BuildTimings;
use std::{collections::BTreeMap, time::Duration};

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
    pub(super) total: Duration,
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
    let diagnostics = Sample::from([
        ("document_build_total", input.document.map(ms)),
        ("cpu_draw_total", Some(ms(input.draw))),
        ("gpu_readback", Some(ms(input.readback))),
        ("taffy_inclusive", Some(ms(input.layout))),
    ]);
    for (name, scope) in [
        ("build_elements", &phases.build),
        ("layout_request", &phases.request_layout),
        ("prepaint", &phases.prepaint),
        ("paint", &phases.paint),
    ] {
        intervals.insert(name, Some(*scope));
    }
    subtract(input.present, input.readback)?;
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
            total: us(60),
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
