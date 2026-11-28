//! Explicitly polled bounded journal. No global worker, periodic invalidation,
//! subscriber lock, or polling work exists before a reader is created.

use crate::process::{Adapter, Poller, Sample};
use serde::Serialize;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Resource {
    /// Logical engine resource category, distinct from DXGI/process memory.
    pub owner: String,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub frame: u64,
    pub phase: String,
    pub process: Sample,
    pub logical_resources: Vec<Resource>,
    pub logical_resources_dropped: u64,
    pub labels_truncated: u64,
}

pub struct Journal {
    poller: Poller,
    cadence: Duration,
    previous: Option<Instant>,
    entries: VecDeque<Entry>,
    capacity: usize,
    dropped: u64,
}

impl Journal {
    /// Cadence is bounded to at most ten publications per second. Readers are
    /// responsible for invoking poll at an engine boundary; this schedules none.
    pub fn new(
        adapter: Option<Adapter>,
        capacity: usize,
        cadence: Duration,
    ) -> Result<Self, &'static str> {
        if capacity == 0 || capacity > 256 || cadence < Duration::from_millis(100) {
            return Err("journal capacity or cadence out of bounds");
        }
        Ok(Self {
            poller: Poller::new(adapter),
            cadence,
            previous: None,
            entries: VecDeque::with_capacity(capacity),
            capacity,
            dropped: 0,
        })
    }

    /// Resource arguments are lazy: no resource walk on a skipped publication.
    pub fn poll(
        &mut self,
        frame: u64,
        phase: &str,
        resources: impl FnOnce() -> Vec<Resource>,
    ) -> bool {
        let now = Instant::now();
        if !self.due(now) {
            return false;
        }
        let mut labels_truncated = u64::from(phase.len() > 64);
        let resources = resources();
        let logical_resources_dropped = resources.len().saturating_sub(256) as u64;
        // Recollect to release a caller vector's excess capacity as well as its
        // excess entries. Each retained journal record has a bounded footprint.
        let logical_resources = resources
            .into_iter()
            .take(256)
            .map(|mut resource| {
                if resource.owner.len() > 128 {
                    labels_truncated += 1;
                }
                // Also release an oversized allocation with a short label.
                resource.owner = bounded_label(&resource.owner, 128);
                resource
            })
            .collect();
        let entry = Entry {
            frame,
            phase: bounded_label(phase, 64),
            process: self.poller.poll(),
            logical_resources,
            logical_resources_dropped,
            labels_truncated,
        };
        self.publish(now, entry);
        true
    }

    fn due(&self, now: Instant) -> bool {
        self.previous
            .is_none_or(|previous| now.duration_since(previous) >= self.cadence)
    }

    fn publish(&mut self, now: Instant, entry: Entry) {
        self.previous = Some(now);
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
            self.dropped += 1;
        }
        self.entries.push_back(entry);
    }

    pub fn latest(&self) -> Option<&Entry> {
        self.entries.back()
    }
    pub fn take(&mut self) -> (Vec<Entry>, u64) {
        (
            self.entries.drain(..).collect(),
            std::mem::take(&mut self.dropped),
        )
    }
}

fn bounded_label(label: &str, max: usize) -> String {
    let mut end = label.len().min(max);
    while !label.is_char_boundary(end) {
        end -= 1;
    }
    label[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_bounds_resources_and_utf8_labels() {
        let mut journal = Journal::new(None, 1, Duration::from_secs(1)).unwrap();
        let long = "é".repeat(100);
        assert!(journal.poll(1, &long, || {
            (0..300)
                .map(|_| Resource {
                    owner: long.clone(),
                    bytes: 8,
                    count: 1,
                })
                .collect()
        }));
        let entry = journal.latest().unwrap();
        assert_eq!(entry.phase.len(), 64);
        assert_eq!(entry.logical_resources.len(), 256);
        assert_eq!(entry.logical_resources[0].owner.len(), 128);
        assert_eq!(entry.logical_resources_dropped, 44);
        assert_eq!(entry.labels_truncated, 257);
    }

    #[test]
    fn cadence_skips_argument_work_and_backpressure_keeps_latest() {
        let mut journal = Journal::new(None, 1, Duration::from_secs(1)).unwrap();
        assert!(journal.poll(1, "idle", Vec::new));
        assert!(!journal.poll(2, "idle", || panic!("skipped resource walk")));
        let start = journal.previous.unwrap();
        assert!(!journal.due(start + Duration::from_millis(999)));
        assert!(journal.due(start + Duration::from_secs(1)));
        let mut entry = journal.latest().unwrap().clone();
        entry.frame = 3;
        journal.publish(start + Duration::from_secs(1), entry);
        let (entries, dropped) = journal.take();
        assert_eq!(dropped, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].frame, 3);
        assert_eq!(journal.take(), (vec![], 0));
    }
}
