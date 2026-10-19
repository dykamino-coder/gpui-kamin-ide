//! Measured clamp paragraph rows and layout probes.

use super::{ClampEntry, ClampLines};
use gpui::{AnyElement, Bounds, IntoElement, Pixels, Styled};

use std::{cell::RefCell, collections::HashMap};

type Rows = HashMap<(u64, u32), Vec<(f32, f32)>>;

/// Actual paragraph rows shared with the document's clamp cut elements.
#[derive(Default)]
pub(crate) struct ParagraphRows(RefCell<Rows>);

impl ParagraphRows {
    pub(crate) fn publish(&self, tag: (u64, u32), rows: Vec<(f32, f32)>) {
        self.0.borrow_mut().insert(tag, rows);
    }

    pub(crate) fn take(&self, key: u64) -> HashMap<u32, Vec<(f32, f32)>> {
        let mut rows = self.0.borrow_mut();
        let tags: Vec<_> = rows.keys().filter(|tag| tag.0 == key).copied().collect();
        tags.into_iter()
            .filter_map(|tag| {
                rows.remove(&tag).map(|mut row| {
                    row.sort_by(|a, b| a.0.total_cmp(&b.0));
                    (tag.1, row)
                })
            })
            .collect()
    }
}

/// Проба строк: канвас в элементе с текстом (или блоке), пишет границы и
/// высоту строки в prepaint своего кадра.
// The probe records paragraph identity, line geometry and clamp constraints.
#[allow(clippy::too_many_arguments)]
pub fn clamp_probe(
    lines: ClampLines,
    line: f32,
    skip_count: bool,
    fixed_height: bool,
    bp_after: f32,
    ends: super::line_state::BoxEnds,
    seq: Option<u32>,
    clamped: Option<usize>,
    span: Option<super::ClampSpan>,
    bt_before: f32,
    positioned: bool,
) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line,
                skip_count,
                fixed_height,
                bp_after,
                end_margin: ends.margin,
                min_end: ends.min_end,
                capped: ends.capped,
                node: ends.node,
                seq,
                clamped,
                empty: false,
                span: span.as_ref().map(|s| s.get()),
                bt_before,
                positioned,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Проба пустой поточной блочной коробки клэмп-контейнера: только её
/// положение. Точка среза после неё (§5.3 «a point between two in-flow
/// block-level sibling boxes») отделяет предыдущую строку от точки, и знака
/// обрыва на той строке нет (`line-clamp-auto-039/032`).
pub fn clamp_empty_probe(lines: ClampLines) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line: 0.0,
                skip_count: false,
                fixed_height: false,
                bp_after: 0.0,
                end_margin: 0.0,
                min_end: 0.0,
                capped: false,
                node: 0,
                seq: None,
                clamped: None,
                empty: true,
                span: None,
                bt_before: 0.0,
                positioned: false,
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

#[cfg(test)]
mod tests {

    #[test]
    fn cuts_take_only_their_document_and_container_rows() {
        let a = crate::document::state::DocState::default();
        let b = crate::document::state::DocState::default();
        a.paragraph_rows
            .publish((7, 0), vec![(20., 25.), (10., 15.)]);
        a.paragraph_rows.publish((8, 0), vec![(30., 35.)]);
        b.paragraph_rows.publish((7, 0), vec![(40., 45.)]);
        assert_eq!(a.paragraph_rows.take(7)[&0], vec![(10., 15.), (20., 25.)]);
        assert!(a.paragraph_rows.take(7).is_empty());
        assert_eq!(a.paragraph_rows.take(8)[&0], vec![(30., 35.)]);
        assert_eq!(b.paragraph_rows.take(7)[&0], vec![(40., 45.)]);
    }
}
