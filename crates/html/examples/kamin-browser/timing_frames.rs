//! Per-frame counters cover the document step and the following draw together.

use super::{counters::Counters, cpu::Cpu, sample::ms};
use gpui::{App, Entity};
use kamin_html::host::document_view::DocumentView;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

struct Started {
    counters: Counters,
    counts: (u64, u64, u64),
    cpu: Cpu,
    at: Instant,
    step: Duration,
}

pub struct Frames {
    page: Option<Entity<DocumentView>>,
    current: Option<Started>,
    pub samples: Vec<Value>,
    pub dropped: usize,
    _why: kamin_html::why::Subscription,
}

impl Frames {
    pub fn new() -> Self {
        let why = kamin_html::why::subscribe();
        kamin_html::why::take();
        Self {
            page: None,
            current: None,
            samples: Vec::new(),
            dropped: 0,
            _why: why,
        }
    }

    pub fn attach(&mut self, page: &Entity<DocumentView>) {
        self.page = Some(page.clone());
    }

    fn counts(&self, cx: &App) -> (u64, u64, u64) {
        self.page.as_ref().map_or((0, 0, 0), |page| {
            let document = page.read(cx).document();
            let (builds, replays) = document.retained_frames();
            (builds, replays, document.builds_inside_draw())
        })
    }

    pub fn begin(&mut self, cx: &App) {
        self.finish(cx);
        self.current = Some(Started {
            counters: Counters::read(),
            counts: self.counts(cx),
            cpu: Cpu::read(),
            at: Instant::now(),
            step: Duration::ZERO,
        });
    }

    pub fn step(&mut self, elapsed: Duration) {
        if let Some(current) = &mut self.current {
            current.step = elapsed;
        }
    }

    pub fn finish(&mut self, cx: &App) {
        let Some(current) = self.current.take() else {
            return;
        };
        let counts = self.counts(cx);
        let why = kamin_html::why::to_json(&kamin_html::why::take());
        if self.samples.len() == 256 {
            self.dropped += 1;
            return;
        }
        self.samples.push(json!({
            "retained_builds": counts.0.saturating_sub(current.counts.0),
            "retained_replays": counts.1.saturating_sub(current.counts.1),
            "builds_inside_draw": counts.2.saturating_sub(current.counts.2),
            "document_step_ms": ms(current.step), "cpu_ms": current.cpu.elapsed_ms(),
            "observed_until_poll_ms": ms(current.at.elapsed()),
            "gpui": current.counters.delta(), "why": why,
        }));
    }
}
