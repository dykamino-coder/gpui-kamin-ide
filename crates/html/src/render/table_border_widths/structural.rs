//! Structural table borders participate in the same grid-edge width as cells.
//! CSS 2.1 §17.6.2–17.6.2.1 includes rows, row groups, columns and column groups.

use super::super::{Element, col_elements, col_role, colgroup_elements};

pub(super) struct Edges<'a> {
    rows: &'a [&'a Element],
    groups: &'a [Option<&'a Element>],
    columns: Vec<Option<&'a Element>>,
    column_groups: Vec<Option<&'a Element>>,
    cols: usize,
    font: f32,
    family: &'a str,
}

impl<'a> Edges<'a> {
    pub(super) fn new(
        table: &'a Element,
        rows: &'a [&'a Element],
        groups: &'a [Option<&'a Element>],
        cols: usize,
        font: f32,
        family: &'a str,
    ) -> Self {
        Self {
            rows,
            groups,
            columns: col_elements(&table.children),
            column_groups: colgroup_elements(&table.children),
            cols,
            font,
            family,
        }
    }

    fn widths(&self, element: &Element) -> [f32; 4] {
        let border = element.style.borders();
        let font = match element.style.font_size {
            Some(crate::value::Len::Px(size)) => size,
            _ => self.font,
        };
        let family = element.style.font_family.as_deref().unwrap_or(self.family);
        [border.top, border.right, border.bottom, border.left]
            .map(|length| crate::metrics::spacing_px(length, family, font))
    }

    pub(super) fn cell(&self, r: usize, c: usize, sr: usize, sc: usize) -> [f32; 4] {
        let mut edges = [0.0_f32; 4];
        // Row top/bottom borders meet the cell only at its span's endpoints.
        if let Some(row) = self.rows.get(r) {
            edges[0] = self.widths(row)[0];
        }
        if let Some(row) = self.rows.get(r + sr - 1) {
            edges[2] = self.widths(row)[2];
        }
        for row in self.rows.iter().skip(r).take(sr) {
            let widths = self.widths(row);
            if c == 0 {
                edges[3] = edges[3].max(widths[3]);
            }
            if c + sc >= self.cols {
                edges[1] = edges[1].max(widths[1]);
            }
        }
        if let Some(group) = self.groups.get(r).copied().flatten() {
            let widths = self.widths(group);
            if r == 0 || self.groups[r - 1].map(|g| g.node_id) != Some(group.node_id) {
                edges[0] = edges[0].max(widths[0]);
            }
            if self
                .groups
                .get(r + sr)
                .copied()
                .flatten()
                .map(|g| g.node_id)
                != Some(group.node_id)
            {
                edges[2] = edges[2].max(widths[2]);
            }
            if c == 0 {
                edges[3] = edges[3].max(widths[3]);
            }
            if c + sc >= self.cols {
                edges[1] = edges[1].max(widths[1]);
            }
        }
        for columns in [&self.columns, &self.column_groups] {
            for index in c..c + sc {
                let Some(element) = columns.get(index).copied().flatten() else {
                    continue;
                };
                let widths = self.widths(element);
                if r == 0 {
                    edges[0] = edges[0].max(widths[0]);
                }
                if r + sr == self.rows.len() {
                    edges[2] = edges[2].max(widths[2]);
                }
                let group = col_role(element) == Some(true);
                let same = |index: usize| {
                    columns
                        .get(index)
                        .copied()
                        .flatten()
                        .is_some_and(|other| other.node_id == element.node_id)
                };
                if index == c && (!group || index == 0 || !same(index - 1)) {
                    edges[3] = edges[3].max(widths[3]);
                }
                if index + 1 == c + sc && (!group || !same(index + 1)) {
                    edges[1] = edges[1].max(widths[1]);
                }
            }
        }
        edges
    }
}
