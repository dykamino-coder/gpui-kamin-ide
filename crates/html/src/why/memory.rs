//! A window owns its process/resource journal. Polling never schedules a frame,
//! invalidates a document, or resets cumulative process counters.

use super::{Entry, MemoryAdapter, MemoryResource, Subscription};
use kamin_profiler::journal::{self, Journal};
use std::time::Duration;

pub struct Reader {
    _subscription: Subscription,
    memory: Journal,
}

impl Reader {
    /// Enable diagnostics explicitly. Pass the actual renderer LUID, or None
    /// for an unavailable GPU reading. No default-adapter guess is performed.
    pub fn new(adapter: Option<MemoryAdapter>) -> Self {
        Self {
            _subscription: super::subscribe(),
            memory: Journal::new(adapter, 8, Duration::from_secs(1))
                .expect("fixed journal limits are valid"),
        }
    }

    /// Invoke at a coherent engine boundary or an existing diagnostic timer.
    /// A skipped sample computes none of the resource callback's arguments.
    pub fn poll_memory(
        &mut self,
        frame: u64,
        phase: &str,
        resources: impl FnOnce() -> Vec<MemoryResource>,
    ) -> bool {
        self.memory.poll(frame, phase, resources)
    }

    pub fn latest_memory(&self) -> Option<&journal::Entry> {
        self.memory.latest()
    }

    /// Walk the live document and window only when a publication is due.
    /// Heap aggregates and logical GPU resources remain separate categories;
    /// neither is added to process commit or DXGI usage.
    pub fn poll_document_memory(
        &mut self,
        frame: u64,
        phase: &str,
        doc: &crate::Document,
        window: &gpui::Window,
        cx: &gpui::App,
    ) -> bool {
        self.poll_memory(frame, phase, || resources(doc, window, cx))
    }

    pub fn take(&mut self) -> Entry {
        let mut entry = super::take();
        (entry.memory, entry.memory_dropped) = self.memory.take();
        entry
    }
}

fn resources(doc: &crate::Document, window: &gpui::Window, cx: &gpui::App) -> Vec<MemoryResource> {
    let mut ops = crate::heap::Ops::default();
    doc.heap_report(&mut ops);
    window.heap_report(cx, &mut |path, pointer, bytes| {
        if pointer.is_null() || ops.once(pointer) {
            ops.at(path, |ops| ops.block(pointer, bytes));
        }
    });
    let mut resources: Vec<_> = ops
        .report()
        .entries()
        .iter()
        .map(|(owner, bytes)| MemoryResource {
            owner: format!("heap/{owner}"),
            bytes: *bytes,
            // One reported aggregate, not an inferred number of allocations.
            count: 1,
        })
        .collect();
    resources.extend(
        super::caches_held()
            .into_iter()
            .map(|(owner, bytes)| MemoryResource {
                owner: format!("heap/process/cache/{owner}"),
                bytes: bytes as u64,
                count: 1,
            }),
    );
    window.gpu_memory(&mut |owner, bytes, _used| {
        resources.push(MemoryResource {
            owner: owner.to_owned(),
            bytes,
            count: 1,
        });
    });
    resources
}
