//! Sizes of a paragraph's joined text and runs, known before they are built.

use crate::text::inline::Piece;

/// Bytes and number of non-empty text pieces; `None` when an atomic inline
/// makes the paragraph a non-text one (`text_and_runs` returns `None` then).
/// The paragraph keeps both buffers: exact sizes replace a doubling series of
/// multi-megabyte buffers on long paragraphs.
pub(super) fn sizes(pieces: &[Piece]) -> Option<(usize, usize)> {
    let mut sizes = (0, 0);
    for piece in pieces {
        match piece {
            Piece::Text { text, .. } if !text.is_empty() => {
                sizes.0 += text.len();
                sizes.1 += 1;
            }
            Piece::Atom(_) => return None,
            _ => {}
        }
    }
    Some(sizes)
}
