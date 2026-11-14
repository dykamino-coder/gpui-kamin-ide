//! Снимок работы одного завершённого кадра; целые значения сохраняются без округления.
//!
//! Образец хранит числа фиксированного размера, а JSON собирается только при выводе отчёта
//! ([`Sample::json`]): иначе карта счётчиков каждого образца (тысячи ключей) жила бы в
//! измеряемой куче до конца прогона и росла с числом правок или перекладок.

use serde_json::Value;

pub(super) struct Capture {
    #[cfg(feature = "work-counters")]
    session: kamin_html::work::Session,
    #[cfg(feature = "work-counters")]
    usage: super::usage::Capture,
}

/// Счётчики работы и ресурсы одного образца.
pub(super) struct Sample {
    cold: bool,
    #[cfg(feature = "work-counters")]
    counters: kamin_html::work::Snapshot,
    #[cfg(feature = "work-counters")]
    usage: super::usage::Sample,
    #[cfg(feature = "work-counters")]
    style_storage: [u64; 6],
}

impl Capture {
    pub(super) fn begin(luid: (i32, u32)) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(feature = "work-counters")]
        let usage = super::usage::Capture::begin(luid)?;
        #[cfg(not(feature = "work-counters"))]
        let _ = luid;
        Ok(Self {
            #[cfg(feature = "work-counters")]
            session: kamin_html::work::Session::begin_measured(
                Some(usage.observer()),
                super::allocations::svg_observer(),
            ),
            #[cfg(feature = "work-counters")]
            usage,
        })
    }

    /// `cold`: the sample ran the document phases (cold and edit samples).
    pub(super) fn finish(self, cold: bool, doc: &kamin_html::Document) -> Sample {
        #[cfg(feature = "work-counters")]
        {
            let usage = self.usage.finish();
            let live = doc.values().doc.style_storage();
            Sample {
                cold,
                counters: self.session.finish(),
                usage,
                style_storage: [
                    live.computed_headers,
                    live.computed_header_bytes(),
                    live.inherited_headers,
                    live.inherited_header_bytes(),
                    live.group_allocations,
                    live.group_payload_bytes,
                ],
            }
        }
        #[cfg(not(feature = "work-counters"))]
        {
            let _ = (self, doc);
            Sample { cold }
        }
    }
}

impl Sample {
    /// The report form: (work counters by phase, process resources).
    pub(super) fn json(&self) -> (Value, Value) {
        #[cfg(feature = "work-counters")]
        {
            use kamin_html::work::{Counter, Phase};
            let cold = self.cold;
            let mut usage = self.usage.json(cold);
            usage["svg_allocation_audit_enabled"] = cfg!(feature = "allocation-audit").into();
            let [
                computed_headers,
                computed_header_bytes,
                inherited_headers,
                inherited_header_bytes,
                group_allocations,
                group_payload_bytes,
            ] = self.style_storage;
            usage["style_storage"] = serde_json::json!({
                "scope": "document initial descendants; nested buffers and Rc control blocks excluded",
                "computed_headers": computed_headers,
                "computed_header_bytes": computed_header_bytes,
                "inherited_headers": inherited_headers,
                "inherited_header_bytes": inherited_header_bytes,
                "unique_group_allocations": group_allocations,
                "unique_group_payload_bytes": group_payload_bytes,
            });
            let mut phases = serde_json::Map::new();
            for (index, name) in Phase::NAMES.iter().enumerate() {
                let values = Counter::NAMES.iter().zip(self.counters.0[index]);
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
            let _ = self.cold;
            (Value::Null, Value::Null)
        }
    }
}

/// The report form of a mode's samples: (work counter samples, process resource samples).
pub(super) fn json(samples: &[Sample]) -> (Vec<Value>, Vec<Value>) {
    samples.iter().map(Sample::json).unzip()
}

#[cfg(all(test, feature = "work-counters"))]
mod tests {
    use super::*;
    #[test]
    fn warm_absence_is_distinct_from_zero_work_and_integers_are_exact() {
        let doc = kamin_html::Document::builder("", "").build().unwrap();
        let capture = Capture::begin((0, 0)).unwrap();
        kamin_html::work::add!(RebuiltNodes, 9_007_199_254_740_993u64);
        let (out, usage) = capture.finish(false, &doc).json();
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

    /// p07 edit: the counter map of every sample stayed in the heap until the report
    /// (+1.39 MB and +10K blocks over 8 edits). Kept samples are numbers in the list the
    /// mode reserves before its first sample, so keeping a sample costs the measuring
    /// thread's heap only what the sample owns (an error message, here the missing GPU
    /// adapter of luid 0), the same after every edit. Taking a sample moves the thread's
    /// heap a little either way: blocks its sampler thread frees count on that thread.
    #[cfg(feature = "allocation-audit")]
    #[test]
    fn kept_samples_do_not_grow_the_measured_heap() {
        use super::super::allocations::thread_live;
        const EDITS: usize = 8;
        let doc = kamin_html::Document::builder("<p>x</p>", "")
            .build()
            .unwrap();
        let sample = || {
            kamin_html::work::add!(RebuiltNodes, 1);
            Capture::begin((0, 0)).unwrap().finish(true, &doc)
        };
        drop(sample());
        let mut kept = Vec::with_capacity(EDITS);
        let mut cost = [0; EDITS];
        for slot in &mut cost {
            let before = thread_live();
            drop(sample());
            let dropped = thread_live() - before;
            let before = thread_live();
            kept.push(sample());
            let owned = kept.last().map_or(0, |s: &Sample| s.usage.owned_bytes()) as i64;
            *slot = thread_live() - before - dropped - owned;
        }
        assert_eq!(cost[1..], [0; EDITS - 1], "{cost:?}");
        let (work, usage) = json(&kept);
        assert_eq!((work.len(), usage.len()), (EDITS, EDITS));
        assert!(work[EDITS - 1]["pipeline_other"]["rebuilt_nodes"].is_u64());
    }
}
