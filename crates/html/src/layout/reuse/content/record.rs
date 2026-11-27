//! The record side of a box's content: wrappers around its build and the
//! records made after the frame's paint (`super`).

use super::super::replay::{PendingReplay, ReplayRecorder};
use super::super::{Counters, FlowReuse, Source};
use super::{Content, ContentRecord, PendingContent, key::key};
use crate::hooks::IntoHooked;
use crate::layout::options::RenderOpts;
use gpui::AnyElement;
use std::rc::Rc;

impl FlowReuse {
    /// Wrap the built content in a record (see the module).
    pub(crate) fn record_content(
        &self,
        content: Content,
        opts: &RenderOpts,
        out: &mut Vec<AnyElement>,
    ) {
        // Its width changes with the window's: the next build of another
        // width would not replay it (Blink `ShouldCreateSubsequence` caches
        // only where a replay is likely).
        if content.follows && self.resized.get() {
            return;
        }
        let end = Counters::read(&opts.cx);
        let images = opts
            .cx
            .vcx()
            .doc
            .images
            .borrow()
            .reads_since(content.images_at);
        let mut items = Vec::with_capacity(out.len());
        for child in std::mem::take(out) {
            let pending = Rc::new(PendingReplay::new(
                None,
                (content.start, end),
                true,
                content.reads.clone(),
                false,
                &self.ink,
                None,
            ));
            items.push(pending.clone());
            out.push(ReplayRecorder::new(child, pending).hooked());
        }
        let mut pending = self.pending_contents.take();
        pending.push(PendingContent {
            owner: content.owner,
            record: ContentRecord {
                key: content.key,
                widths: false,
                at: content.at,
                start: content.start,
                end,
                items: Box::default(),
                images,
            },
            reads: content.reads,
            items,
        });
        self.pending_contents.replace(pending);
    }

    /// The content records of this frame: replayed and finished.
    pub(in crate::layout::reuse) fn finish_contents(&self) {
        let (mut next, mask) = (self.next_contents.take(), self.mask.get());
        for pending in self.pending_contents.take() {
            let PendingContent {
                owner,
                mut record,
                reads,
                items,
            } = pending;
            let finished: Option<Box<[_]>> = items
                .iter()
                .map(|i| i.finish(Source::bare(key(owner)), None, mask).map(Rc::new))
                .collect();
            if let Some(items) = finished {
                record.items = items;
                record.widths = reads.get();
                next.insert(key(owner), Rc::new(record));
            }
        }
        self.store.contents.replace(next);
    }
}
