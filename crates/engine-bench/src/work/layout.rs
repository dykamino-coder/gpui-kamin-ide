//! DevTools layout attribution of a frame: where its layout time went (float
//! band probes, text measurement and shaping, line breaking) and the layout
//! units whose inputs repeated (`kamin_html::work::repeat`), the upper bound
//! of a layout cache by inputs. Summed over the frame's phases.

use serde_json::Value;
#[cfg(feature = "work-counters")]
use serde_json::json;

impl super::Sample {
    pub(crate) fn layout_profile(&self) -> Value {
        #[cfg(feature = "work-counters")]
        {
            let mut profile = profile(&self.counters);
            profile["style_copy_sites"] = sites(&self.clone_sites);
            profile
        }
        #[cfg(not(feature = "work-counters"))]
        {
            Value::Null
        }
    }
}

#[cfg(feature = "work-counters")]
fn profile(counters: &kamin_html::work::Snapshot) -> Value {
    use kamin_html::work::Counter;
    let total = |c: Counter| -> u64 { counters.0.iter().map(|phase| phase[c as usize]).sum() };
    let map = |list: &[Counter]| -> serde_json::Map<String, Value> {
        list.iter()
            .map(|&c| (Counter::NAMES[c as usize].into(), total(c).into()))
            .collect()
    };
    json!({
        "intervals_inclusive_ns": map(&[
            Counter::BandProbeBuildNs,
            Counter::BandProbeLayoutNs,
            Counter::BandIntrinsicBuildNs,
            Counter::BandIntrinsicLayoutNs,
            Counter::MulticolChildLayoutNs,
            Counter::TextMeasureNs,
            Counter::TextShapeNs,
            Counter::LineBreakNs,
            Counter::LayoutRepeatedInFrameNs,
            Counter::LayoutSameAsLastFrameNs,
        ]),
        "counts": map(&[
            Counter::BandProbeCalls,
            Counter::BandIntrinsicCalls,
            Counter::FloatChildBuilds,
            Counter::MeasureCacheHits,
            Counter::MeasureCacheMisses,
            Counter::LayoutCacheHits,
            Counter::LayoutCacheMisses,
            Counter::LayoutUnits,
            Counter::LayoutUnitsRepeatedInFrame,
            Counter::LayoutUnitsSameAsLastFrame,
        ]),
    })
}

/// The largest detached style groups by write location: `file:line`, count, bytes.
#[cfg(feature = "work-counters")]
fn sites(sites: &[(String, u64, u64)]) -> Value {
    let top = sites
        .iter()
        .take(SITES)
        .map(|(site, count, bytes)| json!([site, count, bytes]));
    json!({
        "total_sites": sites.len(),
        "top_by_bytes": top.collect::<Vec<_>>(),
    })
}

/// Write locations kept per sample.
#[cfg(feature = "work-counters")]
const SITES: usize = 24;

#[cfg(all(test, feature = "work-counters"))]
mod tests {
    #[test]
    fn copy_sites_keep_the_largest_and_their_number() {
        let list: Vec<_> = (0..30u64)
            .map(|i| (format!("a.rs:{i}"), 1, 100 - i))
            .collect();
        let value = super::sites(&list);
        assert_eq!(value["total_sites"], 30);
        assert_eq!(
            value["top_by_bytes"].as_array().unwrap().len(),
            super::SITES
        );
        assert_eq!(
            value["top_by_bytes"][0],
            serde_json::json!(["a.rs:0", 1, 100])
        );
    }

    #[test]
    fn layout_profile_sums_phases_and_keeps_repeats() {
        use kamin_html::work::{Phase, Session};
        let first = Session::begin();
        {
            let _layout = kamin_html::work::enter(Phase::LayoutRequest);
            kamin_html::work::add!(TextShapeNs, 7);
            drop(kamin_html::work::repeat(5));
        }
        let carry = first.carry_inputs();
        let _ = first.finish();
        let second = Session::begin();
        second.resume_inputs(carry);
        {
            let _layout = kamin_html::work::enter(Phase::Prepaint);
            kamin_html::work::add!(TextShapeNs, 5);
            drop(kamin_html::work::repeat(5));
        }
        let profile = super::profile(&second.finish());
        assert_eq!(profile["intervals_inclusive_ns"]["text_shape_ns"], 5);
        assert_eq!(profile["counts"]["layout_units"], 1);
        assert_eq!(profile["counts"]["layout_units_same_as_last_frame"], 1);
    }
}
