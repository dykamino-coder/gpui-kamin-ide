//! The media features a build read from the size of the viewport, with the
//! results they had (`MediaReads`).
//!
//! A new viewport size changes the result of a media query only through
//! `width`, `height`, `aspect-ratio` and `orientation` (and their `device-`
//! forms, which are the viewport here). A build records each such feature
//! expression it evaluated together with its result (`record`). The host then
//! asks `flips` on resize: unchanged results mean the rules of the document
//! are the same, and nothing is restyled. Blink keeps the same list
//! (`MediaQueryEvaluator::viewport_dependent_media_query_results_`,
//! `MediaQueryResultFlags`) and re-evaluates it in
//! `StyleEngine::MediaQueryAffectingValueChanged`.

use super::Media;
use crate::style::css::{CustomMedia, SupTri};
use crate::values::ValueCx;
use std::cell::{Cell, RefCell};

/// The most expressions a build keeps; past it any resize restyles.
const CAPACITY: usize = 4096;

thread_local! {
    static RECORD: RefCell<Option<MediaReads>> = const { RefCell::new(None) };
    static READ: Cell<bool> = const { Cell::new(false) };
}

/// The viewport-dependent media feature expressions of one build.
#[derive(Clone, Default)]
pub struct MediaReads {
    seen: Vec<(Box<str>, SupTri)>,
    full: bool,
}

impl MediaReads {
    /// Would a viewport of `media`'s size change the result of any recorded
    /// expression (`true` when the list overflowed).
    pub fn flips(&self, media: &Media, vcx: ValueCx<'_>) -> bool {
        self.full
            || (self.seen.iter())
                .any(|(expr, was)| media.mq_in_parens(expr, CustomMedia::NONE, vcx) != *was)
    }

    /// No expression read the viewport size.
    pub fn is_empty(&self) -> bool {
        !self.full && self.seen.is_empty()
    }
}

/// Run `f` and collect the viewport-dependent expressions it evaluated.
pub fn record<R>(f: impl FnOnce() -> R) -> (R, MediaReads) {
    let outer = RECORD.with(|r| r.replace(Some(MediaReads::default())));
    let out = f();
    let reads = RECORD.with(|r| r.replace(outer)).unwrap_or_default();
    (out, reads)
}

/// A feature value came from the viewport size (`Media::mq_numeric`).
pub(super) fn note() {
    READ.with(|read| read.set(true));
}

/// Evaluate one `<media-in-parens>` leaf and keep it if it read the viewport.
pub(super) fn leaf(expr: &str, eval: impl FnOnce() -> SupTri) -> SupTri {
    let outer = READ.with(|read| read.replace(false));
    let result = eval();
    let read = READ.with(|read| read.replace(outer));
    if read {
        RECORD.with(|r| {
            if let Some(reads) = r.borrow_mut().as_mut() {
                if reads.seen.iter().any(|(e, _)| **e == *expr) {
                    return;
                }
                match reads.seen.len() < CAPACITY {
                    true => reads.seen.push((expr.into(), result)),
                    false => reads.full = true,
                }
            }
        });
    }
    result
}
