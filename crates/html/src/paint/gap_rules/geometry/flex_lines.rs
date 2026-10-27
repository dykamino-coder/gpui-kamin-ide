//! Полосы и состав строк flex из завершённой раскладки, без восстановления по пересечениям.

use super::GapItem;
use gpui::FlexLayout;

#[derive(Default)]
pub(in crate::paint::gap_rules) struct FlexLines {
    pub(in crate::paint::gap_rules) lines: Vec<(f32, f32)>,
    pub(in crate::paint::gap_rules) members: Vec<Vec<GapItem>>,
}

impl FlexLines {
    pub(in crate::paint::gap_rules) fn from_layout(
        origin: (f32, f32),
        layout: &FlexLayout,
    ) -> Self {
        let (a, b) = if layout.is_column {
            origin
        } else {
            (origin.1, origin.0)
        };
        let mut rows: Vec<_> = layout
            .lines
            .iter()
            .map(|line| {
                let band = (
                    a + line.cross_offset,
                    a + line.cross_offset + line.cross_size,
                );
                let members = line
                    .items
                    .iter()
                    .map(|item| GapItem {
                        a0: a + item.cross_offset,
                        a1: a + item.cross_offset + item.cross_size,
                        b0: b + item.main_offset,
                        b1: b + item.main_offset + item.main_size,
                        margin_end: 0.0,
                    })
                    .collect::<Vec<_>>();
                (band, members)
            })
            .collect();
        rows.sort_by(|x, y| x.0.0.total_cmp(&y.0.0));
        let (lines, members) = rows.into_iter().unzip();
        Self { lines, members }
    }
}

#[cfg(test)]
mod tests;
