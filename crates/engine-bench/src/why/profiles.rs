//! Align GPUI and document profile boundaries; preserve gap frames separately.

use serde_json::{Value, json};

pub(super) fn take(instrumentation: &gpui::Instrumentation) -> Value {
    json!({
        "gpu_batches": instrumentation.batches.take_line(),
        "request_layout_kinds": super::element_kinds::to_json(instrumentation.request_layout.take()),
        "layout_inputs": super::layout_inputs::to_json(instrumentation.layout_inputs.take()),
        "native_intervals_ms": super::cold::native_json(instrumentation.startup.take()),
    })
}

pub(super) fn append(entry: &mut Value, profiles: Value) {
    for (key, value) in profiles.as_object().into_iter().flatten() {
        entry[key] = value.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_frames_do_not_inflate_the_next_samples_work() {
        let instrumentation = gpui::Instrumentation::new(gpui::InstrumentationConfig {
            request_layout: true,
            startup: true,
            ..Default::default()
        });
        instrumentation.batches.frame();
        drop(instrumentation.request_layout.enter("host::EmptyFrame"));
        drop(instrumentation.startup.span("gap.native"));
        let between = take(&instrumentation);
        assert!(
            between["gpu_batches"]
                .as_str()
                .unwrap()
                .contains("frames=1")
        );
        assert_eq!(between["request_layout_kinds"]["calls_total"], 1);
        assert_eq!(between["native_intervals_ms"]["gap.native"]["calls"], 1);
        assert!(take(&instrumentation)["gpu_batches"].is_null());
        instrumentation.batches.frame();
        drop(instrumentation.request_layout.enter("document::Root"));
        drop(instrumentation.startup.span("sample.native"));
        let sampled = take(&instrumentation);
        assert!(
            sampled["gpu_batches"]
                .as_str()
                .unwrap()
                .contains("frames=1")
        );
        assert_eq!(sampled["request_layout_kinds"]["calls_total"], 1);
        assert!(sampled["native_intervals_ms"]["gap.native"].is_null());
        assert_eq!(sampled["native_intervals_ms"]["sample.native"]["calls"], 1);
        assert!(take(&instrumentation)["request_layout_kinds"].is_null());
    }
}
