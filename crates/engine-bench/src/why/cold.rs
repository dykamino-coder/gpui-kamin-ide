//! First captured process frame: OS creation, host, document, readback.
//! The OS clock anchors process creation; subsequent intervals use `Instant`.

use serde_json::{Value, json};
use std::{
    cell::RefCell,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::{
    Foundation::FILETIME,
    System::Threading::{GetCurrentProcess, GetProcessTimes},
};

thread_local! {
    static COLD: RefCell<Option<Cold>> = const { RefCell::new(None) };
}

struct Cold {
    host: Instant,
    created_to_host: Duration,
    startup: Value,
    native: Value,
    sample: Option<Instant>,
    captured: Option<Instant>,
}

/// Called only with `--why`; a failed clock anchor never becomes a false zero.
pub(crate) fn start() -> Result<(), Box<dyn std::error::Error>> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // The pseudo handle denotes this process; output buffers outlive the call.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )?;
    }
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    let unix_ticks = ticks
        .checked_sub(116_444_736_000_000_000)
        .ok_or("process creation before Unix epoch")?;
    let birth = UNIX_EPOCH
        + Duration::from_nanos(
            unix_ticks
                .checked_mul(100)
                .ok_or("process creation overflow")?,
        );
    let host = Instant::now();
    let created_to_host = SystemTime::now().duration_since(birth)?;
    COLD.with(|c| {
        *c.borrow_mut() = Some(Cold {
            host,
            created_to_host,
            startup: Value::Null,
            native: Value::Null,
            sample: None,
            captured: None,
        })
    });
    Ok(())
}

pub(crate) fn startup(
    report: Value,
    native: std::collections::BTreeMap<&'static str, gpui::StartupTiming>,
) {
    COLD.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.startup = report;
            c.native = native_json(native);
        }
    });
}

pub(super) fn native_json(
    native: std::collections::BTreeMap<&'static str, gpui::StartupTiming>,
) -> Value {
    native.into_iter().map(|(name, timing)| {
        (name.to_owned(), json!({"calls": timing.calls, "inclusive_ms": timing.elapsed.as_secs_f64() * 1000.}))
    }).collect::<serde_json::Map<_, _>>().into()
}

/// Only the first document: later cold repeats are not process launches.
pub(crate) fn sample_started(at: Instant) {
    COLD.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.sample.get_or_insert(at);
        }
    });
}

pub(crate) fn captured(at: Instant) {
    COLD.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.captured.get_or_insert(at);
        }
    });
}

pub(crate) fn report() -> Value {
    COLD.with(|c| c.borrow().as_ref().map_or(Value::Null, Cold::report))
}

impl Cold {
    fn report(&self) -> Value {
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        let startup_ms = self.startup["total_ms"].as_f64();
        let sample = self.sample;
        let capture = self.captured;
        json!({
            "scope": "OS process creation to first input document's completed readback; includes loader, CLI, empty frame and host preparation",
            "clock": "GetProcessTimes creation FILETIME bridged once through SystemTime to Instant; includes launcher suspended time",
            "process_to_host_ms": ms(self.created_to_host),
            "host_stages_ms": self.startup["stages_ms"],
            "native_intervals_ms": self.native,
            "native_intervals_overlap": true,
            "host_unattributed_ms": sample.zip(startup_ms).map(|(s, startup)| ms(s.duration_since(self.host)) - startup),
            "document_to_capture_ms": sample.zip(capture).map(|(s, p)| ms(p.duration_since(s))),
            "process_to_capture_ms": capture.map(|p| ms(self.created_to_host + p.duration_since(self.host))),
            "present": "unavailable: capture completes before the backend attempts DXGI Present; readback is not a presentation timestamp",
            "intervals_overlap": false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_partitions_creation_through_actual_capture() {
        let entry = Instant::now();
        let cold = Cold {
            host: entry + Duration::from_millis(2),
            created_to_host: Duration::from_millis(5),
            startup: json!({"total_ms": 10., "stages_ms": {"device": 10.}}),
            native: Value::Null,
            sample: Some(entry + Duration::from_millis(15)),
            captured: Some(entry + Duration::from_millis(35)),
        };
        let r = cold.report();
        let sum: f64 = [
            "process_to_host_ms",
            "host_unattributed_ms",
            "document_to_capture_ms",
        ]
        .iter()
        .map(|k| r[*k].as_f64().unwrap())
        .sum::<f64>()
            + 10.;
        assert_eq!(sum, 38.);
        assert_eq!(r["process_to_capture_ms"], sum);
        assert_eq!(r["document_to_capture_ms"], 20.);
    }

    #[test]
    fn repeated_cold_samples_keep_first_process_frame() {
        let at = Instant::now();
        COLD.with(|c| {
            *c.borrow_mut() = Some(Cold {
                host: at,
                created_to_host: Duration::ZERO,
                startup: Value::Null,
                native: Value::Null,
                sample: None,
                captured: None,
            })
        });
        sample_started(at + Duration::from_millis(1));
        captured(at + Duration::from_millis(3));
        sample_started(at + Duration::from_millis(10));
        captured(at + Duration::from_millis(20));
        assert_eq!(report()["process_to_capture_ms"], 3.);
        assert_eq!(report()["document_to_capture_ms"], 2.);
        startup(
            json!({"total_ms": 1.}),
            [(
                "platform.device",
                gpui::StartupTiming {
                    calls: 1,
                    elapsed: Duration::from_millis(2),
                },
            )]
            .into(),
        );
        assert_eq!(
            report()["native_intervals_ms"]["platform.device"]["calls"],
            1
        );
        assert_eq!(
            report()["native_intervals_ms"]["platform.device"]["inclusive_ms"],
            2.
        );
        assert_eq!(report()["native_intervals_overlap"], true);
        COLD.with(|c| *c.borrow_mut() = None);
    }
}
