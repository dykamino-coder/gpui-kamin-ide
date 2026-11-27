//! The image request log a layout reads through the document values: a
//! layout records and replays image reads without borrowing the store itself.

use super::store::ImageRead;
use crate::values::DocumentValues;

impl DocumentValues {
    /// Where the image request log stands before a build reads images; `first`
    /// (the frame's first such build) empties the log of the previous frame.
    pub fn image_log_mark(&self, first: bool) -> usize {
        let images = self.images.borrow();
        if first {
            images.end_frame_log();
        }
        images.log_position()
    }

    /// The reads of a replayed build hold: each image keeps the state read
    /// (`ImageStore::replay_reads`).
    pub fn replay_image_reads(&self, reads: &[ImageRead]) -> bool {
        self.images.borrow().replay_reads(reads)
    }
}
