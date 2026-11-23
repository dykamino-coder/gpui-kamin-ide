//! DevTools CPU paint attribution, collected after a sample completes.

use serde_json::Value;
#[cfg(feature = "work-counters")]
use serde_json::json;

impl super::Sample {
    pub(crate) fn paint_profile(&self) -> Value {
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
    use kamin_html::work::{Counter, Phase};
    let paint = &counters.0[Phase::Paint as usize];
    let intervals: serde_json::Map<String, Value> = [
        Counter::CacheLookupNs,
        Counter::CacheInsertNs,
        Counter::CacheEvictNs,
        Counter::GradientRequestNs,
        Counter::GradientRasterNs,
        Counter::PaintUploadNs,
        Counter::PaintBackgroundNs,
        Counter::PaintGroupNs,
    ]
    .into_iter()
    .map(|c| (Counter::NAMES[c as usize].into(), paint[c as usize].into()))
    .collect();
    json!({
        "intervals_inclusive_ns": intervals,
        "gradient_rasters": paint[Counter::GradientRasters as usize],
        "gradient_raster_pixels": paint[Counter::GradientRasterPixels as usize],
        "rounded_mask_rasters": paint[Counter::RoundedMaskRasters as usize],
    })
}

#[cfg(all(test, feature = "work-counters"))]
mod tests {
    #[test]
    fn paint_profile_keeps_entry_phase_and_exact_pixel_count() {
        use kamin_html::work::{Counter, Phase, Session};
        let session = Session::begin();
        {
            let _phase = kamin_html::work::enter(Phase::Paint);
            kamin_html::work::add!(GradientRasterNs, 123456789);
            kamin_html::work::add!(GradientRasterPixels, 9007199254740993u64);
        }
        let profile = super::profile(&session.finish());
        assert_eq!(
            profile["intervals_inclusive_ns"][Counter::NAMES[Counter::GradientRasterNs as usize]],
            123456789u64
        );
        assert_eq!(profile["gradient_raster_pixels"], 9007199254740993u64);
    }
}
