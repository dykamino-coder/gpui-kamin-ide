//! Последовательные интервалы запуска до первого GPU-кадра, отдельно от документа.

use std::{collections::BTreeMap, time::Instant};

pub(super) struct Startup {
    previous: Instant,
    stages: BTreeMap<&'static str, f64>,
}

impl Startup {
    pub(super) fn new() -> Self {
        Self {
            previous: Instant::now(),
            stages: BTreeMap::new(),
        }
    }

    pub(super) fn mark(&mut self, name: &'static str) {
        let now = Instant::now();
        self.stages
            .insert(name, (now - self.previous).as_secs_f64() * 1000.);
        self.previous = now;
    }

    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"stages_ms": self.stages,
            "total_ms": self.stages.values().sum::<f64>(),
            "intervals_overlap": false,
            "scope": "host launch through initial empty GPU readback; excludes executable loader and CLI parsing"})
    }
}
