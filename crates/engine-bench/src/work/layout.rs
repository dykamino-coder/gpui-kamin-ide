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
            profile(&self.counters)
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

#[cfg(all(test, feature = "work-counters"))]
mod tests {
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
        drop(first.finish());
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
