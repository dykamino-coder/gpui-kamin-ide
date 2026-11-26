//! Existing GPUI cumulative counters; deltas wrap exactly like their u32 sources.

use std::sync::atomic::Ordering;

const N: usize = 14;
pub struct Counters([u32; N]);

impl Counters {
    pub fn read() -> Self {
        use gpui::frame_perf::*;
        Self(
            [
                &DRAW_US,
                &PREPAINT_US,
                &PAINT_US,
                &TAFFY_US,
                &MEASURE_US,
                &PRESENT_US,
                &SWAP_US,
                &SHAPE_US,
                &FRAMES,
                &LAYOUT_RUNS,
                &ROOT_LAYOUTS,
                &ISOLATED_FRAMES,
                &SHAPED_LINES,
                &CACHED_LINES,
            ]
            .map(|counter| counter.load(Ordering::Relaxed)),
        )
    }

    pub fn delta(&self) -> serde_json::Value {
        let after = Self::read();
        let names = [
            "draw_ms",
            "prepaint_ms",
            "paint_ms",
            "layout_taffy_ms",
            "measure_ms",
            "raster_submit_ms",
            "swap_ms",
            "shape_ms",
            "frames",
            "layout_runs",
            "root_layouts",
            "isolated_frames",
            "shaped_lines",
            "cached_lines",
        ];
        let mut out = serde_json::Map::new();
        for (i, name) in names.into_iter().enumerate() {
            let value = after.0[i].wrapping_sub(self.0[i]);
            out.insert(
                name.into(),
                if i < 8 {
                    serde_json::json!(f64::from(value) / 1000.)
                } else {
                    serde_json::json!(value)
                },
            );
        }
        serde_json::Value::Object(out)
    }
}
