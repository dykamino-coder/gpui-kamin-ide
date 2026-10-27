//! Exhaustive callback refusal, exact resource boundaries and owner lifetimes.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Callback,
    Work,
    Names,
    Storage,
}

#[derive(Default)]
struct Metrics {
    callbacks: AtomicUsize,
    work: AtomicUsize,
    names: AtomicUsize,
    live: AtomicUsize,
    peak: AtomicUsize,
    read: AtomicUsize,
}

struct Ticket {
    metrics: Arc<Metrics>,
    bytes: usize,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.metrics.live.fetch_sub(self.bytes, Relaxed);
    }
}

#[derive(Clone)]
struct Policy {
    metrics: Arc<Metrics>,
    fail_callback: usize,
    work_limit: usize,
    name_limit: usize,
    live_limit: usize,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            metrics: Arc::default(),
            fail_callback: usize::MAX,
            work_limit: usize::MAX,
            name_limit: usize::MAX,
            live_limit: usize::MAX,
        }
    }
}
impl Policy {
    fn callback(&self) -> Result<(), Refusal> {
        if self.metrics.callbacks.fetch_add(1, Relaxed) == self.fail_callback {
            Err(Refusal::Callback)
        } else {
            Ok(())
        }
    }
}
impl Admission for Policy {
    type Error = Refusal;
    fn work(&self, n: usize) -> Result<(), Refusal> {
        self.callback()?;
        let total = self
            .metrics
            .work
            .load(Relaxed)
            .checked_add(n)
            .ok_or(Refusal::Work)?;
        if total > self.work_limit {
            return Err(Refusal::Work);
        }
        self.metrics.work.store(total, Relaxed);
        Ok(())
    }
    fn name(&self, _: &str) -> Result<(), Refusal> {
        self.callback()?;
        let total = self.metrics.names.load(Relaxed) + 1;
        if total > self.name_limit {
            return Err(Refusal::Names);
        }
        self.metrics.names.store(total, Relaxed);
        Ok(())
    }
    fn storage(&self, bytes: usize) -> Result<Lease, Refusal> {
        self.callback()?;
        let total = self
            .metrics
            .live
            .load(Relaxed)
            .checked_add(bytes)
            .ok_or(Refusal::Storage)?;
        if total > self.live_limit {
            return Err(Refusal::Storage);
        }
        self.metrics.live.store(total, Relaxed);
        self.metrics.peak.fetch_max(total, Relaxed);
        Ok(Some(Arc::new(Ticket {
            metrics: self.metrics.clone(),
            bytes,
        })))
    }
}

struct Source {
    remaining: &'static [u8],
    metrics: Arc<Metrics>,
}
impl Read for Source {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = buffer.len().min(self.remaining.len());
        buffer[..n].copy_from_slice(&self.remaining[..n]);
        self.remaining = &self.remaining[n..];
        self.metrics.read.fetch_add(n, Relaxed);
        Ok(n)
    }
}

const XML: &str = "<?xml version='1.0' encoding='UTF-8'?>\
    <!DOCTYPE r [<!ENTITY text 'abc'><!ENTITY nested '&text;'>]>\
    <?target data?><r xmlns='urn:r' xmlns:p='urn:p' p:attr='&text;' second='value'>\
    KEEP<child/><!--comment--><![CDATA[<cdata>]]>&nested;&#x41;</r>";

fn parse(source: &'static str, policy: Policy) -> Result<(), Failure<Refusal>> {
    let metrics = policy.metrics.clone();
    let mut reader = AbortableReader::new(
        Source {
            remaining: source.as_bytes(),
            metrics: metrics.clone(),
        },
        ParserConfig2::new().ignore_comments(false),
        policy,
    )?;
    while let Some(event) = reader.next() {
        if let Err(failure) = event {
            // Core storage is gone while the terminal reader and source are alive.
            assert_eq!(metrics.live.load(Relaxed), 0);
            let stopped = (metrics.callbacks.load(Relaxed), metrics.read.load(Relaxed));
            for _ in 0..3 {
                assert!(reader.next().is_none());
            }
            assert_eq!(
                stopped,
                (metrics.callbacks.load(Relaxed), metrics.read.load(Relaxed))
            );
            return Err(failure);
        }
    }
    assert_eq!(metrics.live.load(Relaxed), 0);
    Ok(())
}

mod scenarios;
