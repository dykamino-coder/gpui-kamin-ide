//! Возможности разрыва, экспортируемые измеренными контекстами форматирования.
//!
//! Координаты — в логической блочной оси владельца. Параллельные потоки дают
//! возможности вместе; разрез не может пересекать ни одну измеренную строку или
//! монолитный бокс (CSS Break 3, разделы 4.1 и 4.3). Деревья layout остаются
//! временными.

pub(crate) mod blocks;
mod measurements;
mod stack;
pub(crate) use measurements::{FlowMeasure, FlowNode};

/// Whether the line boxes inside `e` are line boxes of the block flow that
/// contains `e` (css-break-3 §4.3 class B): a block container in normal flow
/// or a float (BandFlow), or an inline box. Tables, flex and grid containers
/// fragment by their own rows and lines (css-break-3 §4.3 class A/C between
/// rows, items and tracks), multicol containers by their own columns;
/// monolithic boxes (§4.1) and out-of-flow boxes take no part in it.
pub(crate) fn flow_lines(e: &crate::dom::Element) -> bool {
    use crate::style::computed::Display;
    matches!(
        e.style.display,
        None | Some(Display::Block) | Some(Display::ListItem) | Some(Display::Contents)
    ) && e.style.webkit_box != Some(true)
        && !matches!(
            e.tag.as_str(),
            "table" | "caption" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th"
        )
        && !crate::layout::multicol::spanner::multicol_container(&e.style)
        && !crate::layout::fragment::table_bands::table_box(e)
        && !crate::layout::fragment::clone::monolith_box(e)
        && !crate::layout::box_kind::out_of_flow(&e.style)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FlowGeometry {
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) solid: Vec<(f32, f32)>,
    /// Original float identities and boxes, never temporary LayoutIds.
    pub(crate) floats: Vec<FloatGeometry>,
}

/// Float margins remain unbreakable and are preserved at fragmentainer starts.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatGeometry {
    pub(crate) id: u64,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) margins: [f32; 2],
    /// First legal internal break of a non-monolithic float, in owner coordinates.
    pub(crate) first_break: Option<f32>,
}

impl FlowGeometry {
    pub(crate) fn merge(self, cuts: &mut Vec<(f32, f32)>, solid: &mut Vec<(f32, f32)>) {
        if self.cuts.is_empty() {
            return;
        }
        let mut merged = self;
        merged.cuts.append(cuts);
        merged.solid.append(solid);
        merged.normalize();
        *cuts = merged.cuts;
        *solid = merged.solid;
    }

    pub(crate) fn append(&mut self, other: &Self, offset: f32) {
        self.floats
            .extend(other.floats.iter().map(|g| FloatGeometry {
                top: g.top + offset,
                bottom: g.bottom + offset,
                first_break: g.first_break.map(|b| b + offset),
                ..*g
            }));
        self.cuts
            .extend(other.cuts.iter().map(|&(a, b)| (a + offset, b + offset)));
        self.solid
            .extend(other.solid.iter().map(|&(a, b)| (a + offset, b + offset)));
    }

    pub(crate) fn normalize(&mut self) {
        use crate::tolerance::COORD_EPS;
        self.cuts.retain(|&(a, _)| {
            a.is_finite()
                && a > COORD_EPS
                && !self
                    .solid
                    .iter()
                    .any(|&(s, e)| s + COORD_EPS < a && a < e - COORD_EPS)
        });
        self.cuts
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        self.cuts
            .dedup_by(|a, b| (a.0 - b.0).abs() < COORD_EPS && (a.1 - b.1).abs() < COORD_EPS);
    }

    /// Keep the first `orphans` and the last `widows` line boxes of one inline
    /// formatting context together (css-break-3 §4.4; Blink
    /// `kBreakAppealViolatingOrphansAndWidows` in `block_layout_algorithm.cc`):
    /// the break opportunities between them are removed and their lines form
    /// one unbreakable range. `self` holds the lines' solids in order, one per
    /// line (`lines`); a break before the first or after the last line stays.
    pub(crate) fn keep_lines(&mut self, orphans: usize, widows: usize) {
        use crate::tolerance::COORD_EPS;
        let n = self.solid.len();
        if n < 2 {
            return;
        }
        // Break after line `k` (1-based) is legal when `k` lines stay before
        // it and `n - k` after it.
        let legal = |k: usize| k == 0 || k >= n || (k >= orphans.max(1) && n - k >= widows.max(1));
        let lines = std::mem::take(&mut self.solid);
        let mut groups: Vec<(f32, f32)> = Vec::new();
        for (i, &(a, b)) in lines.iter().enumerate() {
            match groups.last_mut() {
                Some(g) if !legal(i) => g.1 = b,
                _ => groups.push((a, b)),
            }
        }
        // A cut inside a group (between its lines, or in clearance before one
        // of them) breaks between kept lines.
        self.cuts.retain(|&(a, _)| {
            !groups
                .iter()
                .any(|&(s, e)| s + COORD_EPS < a && a < e - COORD_EPS)
        });
        self.solid = groups;
    }

    pub(crate) fn lines(heights: impl IntoIterator<Item = f32>) -> Self {
        let mut geometry = Self::default();
        let mut y = 0.0;
        for height in heights {
            let end = y + height;
            geometry.solid.push((y, end));
            geometry.cuts.push((end, end));
            y = end;
        }
        geometry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_lines_only_break_at_shared_edges() {
        let mut geometry = FlowGeometry::lines([10.0, 20.0]);
        geometry.append(&FlowGeometry::lines([15.0, 15.0]), 0.0);
        geometry.normalize();
        assert_eq!(geometry.cuts, vec![(30.0, 30.0)]);
    }

    #[test]
    fn orphans_and_widows_keep_edge_lines_together() {
        let mut geometry = FlowGeometry::lines([10.0; 5]);
        geometry.keep_lines(2, 2);
        assert_eq!(
            geometry.solid,
            vec![(0.0, 20.0), (20.0, 30.0), (30.0, 50.0)]
        );
        assert_eq!(
            geometry.cuts,
            vec![(20.0, 20.0), (30.0, 30.0), (50.0, 50.0)]
        );
        // Too few lines: the paragraph is one range, broken only around it.
        let mut geometry = FlowGeometry::lines([10.0; 3]);
        geometry.keep_lines(2, 2);
        assert_eq!(geometry.solid, vec![(0.0, 30.0)]);
        assert_eq!(geometry.cuts, vec![(30.0, 30.0)]);
    }

    #[test]
    fn float_translation_preserves_unbreakable_margins() {
        let source = FlowGeometry {
            floats: vec![FloatGeometry {
                id: 7,
                top: 40.0,
                bottom: 60.0,
                margins: [40.0, 40.0],
                first_break: Some(50.0),
            }],
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        geometry.append(&source, 15.0);
        let g = geometry.floats[0];
        assert_eq!((g.id, g.top, g.bottom), (7, 55.0, 75.0));
        assert_eq!(g.first_break, Some(65.0));
        assert_eq!(
            (g.top - g.margins[0], g.bottom + g.margins[1]),
            (15.0, 115.0)
        );
    }

    #[test]
    fn translated_rows_include_inline_box_growth() {
        let mut geometry = FlowGeometry::default();
        geometry.append(&FlowGeometry::lines([10.0, 30.0, 10.0]), 5.0);
        geometry.normalize();
        assert_eq!(
            geometry.cuts,
            vec![(15.0, 15.0), (45.0, 45.0), (55.0, 55.0)]
        );
        assert_eq!(geometry.solid[1], (15.0, 45.0));
    }
}
