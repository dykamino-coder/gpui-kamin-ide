//! Requests that outlive a document revision, and what a replayed record of
//! the layout read from them.
//!
//! A new revision of the same document (a DOM edit, a script mutation) keeps
//! its image requests: a ready image stays ready and a pending load goes on,
//! so a content whose images did not change replays (`layout::reuse`). A
//! request no revision asked for or replayed since the previous one is freed
//! at the next (`renew`): an image taken off the document releases its entry
//! after one revision. Blink keeps an `ImageResourceContent` while an element
//! holds it and the memory cache by its URL, not per document lifetime.

use super::{ImageId, ImageState, ImageStore};
use std::sync::atomic::Ordering::Relaxed;

/// One image as a record read it: its handle and the state it had
/// (`ImageStore::image_revision`).
pub type ImageRead = (ImageId, u64);

impl ImageStore {
    /// A new document revision: requests used since the previous one stay,
    /// the others are freed; jobs in flight go on.
    pub(super) fn renew(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        let stale: Vec<ImageId> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, cell)| {
                let entry = cell.entry.as_ref()?;
                let fresh = self.epoch.wrapping_sub(entry.used.load(Relaxed)) <= 1;
                (!fresh).then_some(ImageId {
                    slot: slot as u32,
                    generation: cell.generation,
                })
            })
            .collect();
        for id in stale {
            self.queue.retain(|queued| queued.id != id);
            self.free_slot(id);
        }
        // SVG rasters stay: their keys hold every input by content and resource
        // revision, and their cache has its own budget.
        self.revision += 1;
    }

    /// The request `id` is used in this revision.
    pub(super) fn touch(&self, id: ImageId) {
        if let Some(entry) = self.entry(id) {
            entry.used.store(self.epoch, Relaxed);
            self.log().push(id);
        }
    }

    /// The state of request `id` as one number: its attempt and whether it
    /// is still pending. A load, a failure or a revalidation changes it.
    pub fn image_revision(&self, id: ImageId) -> Option<u64> {
        let entry = self.entry(id)?;
        let pending = matches!(entry.state, ImageState::Pending);
        Some(entry.attempt * 2 + u64::from(pending))
    }

    /// Live requests of the document.
    pub fn requests(&self) -> usize {
        self.slots.iter().filter(|s| s.entry.is_some()).count()
    }

    /// Position in this frame's request log (`reads_since`).
    pub fn log_position(&self) -> usize {
        self.log().len()
    }

    /// The images requested or replayed since log position `at`, once each.
    pub fn reads_since(&self, at: usize) -> Box<[ImageRead]> {
        let mut ids: Vec<ImageId> = self.log().get(at..).unwrap_or_default().to_vec();
        ids.sort_unstable_by_key(|id| (id.slot, id.generation));
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| Some((id, self.image_revision(id)?)))
            .collect()
    }

    /// Every image of `reads` is in the state it was read in; each is used
    /// again in this revision, as the replay draws it.
    pub fn replay_reads(&self, reads: &[ImageRead]) -> bool {
        if !reads
            .iter()
            .all(|&(id, rev)| self.image_revision(id) == Some(rev))
        {
            return false;
        }
        for &(id, _) in reads {
            self.touch(id);
        }
        true
    }

    /// This frame's request log; a poisoned lock keeps its list.
    pub(super) fn log(&self) -> std::sync::MutexGuard<'_, Vec<ImageId>> {
        self.log.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The frame is over: its request log starts empty in the next one.
    pub fn end_frame_log(&self) {
        self.log().clear();
    }
}
