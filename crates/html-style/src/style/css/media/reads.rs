//! The media features a build read from the size of the viewport, with the
//! results they had (`MediaReads`).
//!
//! A new viewport size changes the result of a media query only through
//! `width`, `height`, `aspect-ratio` and `orientation` (and their `device-`
//! forms, which are the viewport here). A build records each feature
//! expression that names one of them together with its result (`record`),
//! in the values of its document. The host then asks `flips` on resize:
//! unchanged results mean the rules of the document are the same, and
//! nothing is restyled. Blink keeps the same list
//! (`MediaQueryEvaluator::viewport_dependent_media_query_results_`,
//! `MediaQueryResultFlags`) and re-evaluates it in
//! `StyleEngine::MediaQueryAffectingValueChanged`.

use super::Media;
use crate::style::css::{CustomMedia, SupTri};
use crate::values::{DocumentValues, ValueCx};

/// The most expressions a build keeps; past it any resize restyles.
const CAPACITY: usize = 4096;

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

/// Run `f` and collect the viewport-dependent expressions it evaluated in
/// the document of `doc`.
pub fn record<R>(doc: &DocumentValues, f: impl FnOnce() -> R) -> (R, MediaReads) {
    let outer = doc
        .traversal
        .media_record
        .replace(Some(MediaReads::default()));
    let out = f();
    let reads = doc
        .traversal
        .media_record
        .replace(outer)
        .unwrap_or_default();
    (out, reads)
}

/// The features whose value is the size of the viewport (mediaqueries-4
/// §4.1–§4.4, the deprecated `device-` forms of §B), with their range forms.
fn reads_viewport(expr: &str) -> bool {
    const SIZE: [&str; 4] = ["width", "height", "aspect-ratio", "orientation"];
    (expr.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')))
        .map(|word| {
            let word = word.trim_start_matches("min-").trim_start_matches("max-");
            word.strip_prefix("device-").unwrap_or(word)
        })
        .any(|word| SIZE.iter().any(|name| word.eq_ignore_ascii_case(name)))
}

/// Evaluate one `<media-in-parens>` leaf and keep it if it names a feature of
/// the viewport size. Over-matching (a word in a value) only keeps an
/// expression whose result cannot flip.
pub(super) fn leaf(expr: &str, vcx: ValueCx<'_>, eval: impl FnOnce() -> SupTri) -> SupTri {
    let result = eval();
    if reads_viewport(expr)
        && let Some(reads) = vcx.doc.traversal.media_record.borrow_mut().as_mut()
        && !reads.seen.iter().any(|(e, _)| **e == *expr)
    {
        match reads.seen.len() < CAPACITY {
            true => reads.seen.push((expr.into(), result)),
            false => reads.full = true,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::reads_viewport;

    #[test]
    fn only_features_of_the_viewport_size_are_kept() {
        for expr in [
            "(width < 300px)",
            "(min-width: 600px)",
            "(400px <= max-height)",
            "(device-aspect-ratio: 16/9)",
            "(orientation: portrait)",
        ] {
            assert!(reads_viewport(expr), "{expr}");
        }
        for expr in [
            "(color)",
            "(prefers-color-scheme: dark)",
            "(hover: hover)",
            "(resolution: 1dppx)",
        ] {
            assert!(!reads_viewport(expr), "{expr}");
        }
    }
}
