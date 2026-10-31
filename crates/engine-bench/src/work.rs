//! Снимок работы одного завершённого кадра; целые значения сохраняются без округления.

use serde_json::Value;

pub(super) struct Capture {
    #[cfg(feature = "work-counters")]
    session: kamin_html::work::Session,
    #[cfg(feature = "work-counters")]
    usage: super::usage::Capture,
}

impl Capture {
    pub(super) fn begin(luid: (i32, u32)) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "work-counters")]
        let usage = super::usage::Capture::begin(luid)?;
        #[cfg(not(feature = "work-counters"))]
        let _ = luid;
        Ok(Self {
            #[cfg(feature = "work-counters")]
            session: kamin_html::work::Session::begin_observed(Some(usage.observer())),
            #[cfg(feature = "work-counters")]
            usage,
        })
    }

    pub(super) fn finish(self, cold: bool) -> (Value, Value) {
        #[cfg(feature = "work-counters")]
        {
            use kamin_html::work::{Counter, Phase};
            let usage = self.usage.finish(cold);
            let snapshot = self.session.finish();
            let mut phases = serde_json::Map::new();
            for (index, name) in Phase::NAMES.iter().enumerate() {
                let values = Counter::NAMES.iter().zip(snapshot.0[index]);
                phases.insert(
                    (*name).into(),
                    if !cold && index <= Phase::DocumentOther as usize {
                        Value::Null
                    } else {
                        Value::Object(
                            values
                                .map(|(key, value)| ((*key).into(), value.into()))
                                .collect(),
                        )
                    },
                );
            }
            (Value::Object(phases), usage)
        }
        #[cfg(not(feature = "work-counters"))]
        {
            let _ = cold;
            (Value::Null, Value::Null)
        }
    }
}

#[cfg(all(test, feature = "work-counters"))]
mod tests {
    use super::*;
    #[test]
    fn warm_absence_is_distinct_from_zero_work_and_integers_are_exact() {
        let capture = Capture::begin((0, 0)).unwrap();
        kamin_html::work::add!(RebuiltNodes, 9_007_199_254_740_993u64);
        let (out, usage) = capture.finish(false);
        assert!(usage["phases"]["parse"].is_null());
        assert!(out["parse"].is_null());
        assert_eq!(out["paint"]["rebuilt_nodes"], 0);
        let phase_cpu: u64 = usage["phases"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|phase| phase["process_cpu_100ns"].as_u64())
            .sum();
        assert_eq!(phase_cpu, usage["process_cpu_100ns"].as_u64().unwrap());
        assert_eq!(
            out["pipeline_other"]["rebuilt_nodes"],
            9_007_199_254_740_993u64
        );
    }
}
