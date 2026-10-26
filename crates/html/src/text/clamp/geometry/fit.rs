//! Точка среза в порядке дерева коробок: переполнение коробок заданной высоты и видимая
//! протяжённость. Строки, вышедшие за низ коробки заданной высоты, для среза сидят на её конце
//! (css-overflow-4 §5.3; Blink смотрит на block-end коробки, а не на её строки); нижний край
//! содержимого ДО точки — для абзацев раньше точки и позиционированных коробок, чьё
//! переполнение остаётся видимым.

use super::ClampEntry;
use crate::tolerance::LINE_FIT_EPS;

/// Block-axis spans of boxes with a definite height (incl. zero height).
pub(super) fn fixed_blocks(entries: &[ClampEntry]) -> Vec<(f32, f32, (u32, u32))> {
    entries
        .iter()
        .filter(|e| e.line <= 0.0 && e.fixed_height && !e.skip_count)
        .filter_map(|e| {
            let y0 = f32::from(e.bounds.origin.y);
            Some((y0, y0 + f32::from(e.bounds.size.height), e.span?))
        })
        .collect()
}

/// Rows overflowing the end of a box with a definite height sit, for the
/// clamp, at that box's end: the clamp point is found in box-tree order,
/// and such a box ends where its size says (css-overflow-4 §5.3; Blink
/// checks the box's block-end offset, not its overflowing lines —
/// `line-clamp-auto-040/047`). Parked rows never pull the cut up.
pub(super) fn park_overflow(
    rows: Vec<(f32, f32)>,
    e: &ClampEntry,
    fixed: &[(f32, f32, (u32, u32))],
) -> Vec<(f32, f32)> {
    if e.skip_count {
        return rows;
    }
    let Some(seq) = e.seq else { return rows };
    let ey1 = f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height);
    // The fixed box is an ancestor of the paragraph in box-tree order.
    let Some(end) = fixed
        .iter()
        .filter(|(_, y1, (a, b))| (*a..*b).contains(&seq) && ey1 > y1 + LINE_FIT_EPS)
        .map(|(_, y1, _)| *y1)
        .reduce(f32::min)
    else {
        return rows;
    };
    rows.into_iter()
        // A row straddling the box end is overflow of the box too: it ends
        // where the box does (`line-clamp-auto-063`).
        .map(|(a, b)| (a.min(end), b.min(end)))
        .collect()
}

/// Low edge (from `top`) of the content before a clamp point between
/// blocks, `next` being the first paragraph after it: earlier paragraphs
/// and positioned boxes whose containing block precedes the point keep
/// their overflow visible (`line-clamp-with-abspos-023`).
pub(super) fn extent_before_next(next: u32, entries: &[ClampEntry], top: f32) -> Option<f32> {
    entries
        .iter()
        .filter(|e| {
            (!e.skip_count && e.line > 0.0 && e.seq.is_some_and(|s| s < next))
                || (e.positioned && e.span.is_some_and(|(a, _)| a < next))
        })
        .map(|e| f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height))
        .max_by(|a, b| a.total_cmp(b))
        .map(|b| b - top)
}

/// Low edge (from `top`) of the content before the clamp point in
/// box-tree order: paragraphs numbered before the clamp paragraph and the
/// kept rows of that paragraph keep their own overflow visible.
pub(super) fn extent_before_point(
    point: Option<(f32, (u32, usize))>,
    entries: &[ClampEntry],
    split: &dyn Fn(&ClampEntry) -> Vec<(f32, f32)>,
    top: f32,
) -> Option<f32> {
    let (_, (s, k)) = point?;
    // A box right after the clamp paragraph precedes the clamp point only
    // when that paragraph keeps all its rows (the point is then before the
    // next line: `line-clamp-with-abspos-023` vs `-021`).
    let whole = entries
        .iter()
        .filter(|e| e.seq == Some(s) && !e.skip_count)
        .all(|e| split(e).len() <= k);
    let positioned = entries
        .iter()
        .filter(|e| e.positioned)
        .filter(|e| e.span.is_some_and(|(a, _)| a <= s || (a == s + 1 && whole)))
        .map(|e| f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height));
    entries
        .iter()
        .filter(|e| !e.skip_count && e.line > 0.0)
        .filter_map(|e| {
            let sq = e.seq?;
            let y0 = f32::from(e.bounds.origin.y);
            match sq.cmp(&s) {
                std::cmp::Ordering::Less => Some(y0 + f32::from(e.bounds.size.height)),
                std::cmp::Ordering::Equal => split(e).get(k.max(1) - 1).map(|r| r.1),
                std::cmp::Ordering::Greater => None,
            }
        })
        .chain(positioned)
        .max_by(|a, b| a.total_cmp(b))
        .map(|b| b - top)
}
