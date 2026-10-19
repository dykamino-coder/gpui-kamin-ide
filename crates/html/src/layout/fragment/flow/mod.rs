//! Break opportunities exported by measured formatting contexts.
//!
//! Coordinates use the owner's logical block axis. Parallel flows contribute
//! opportunities together; a cut may not cross any measured line or monolithic
//! box (CSS Break 3 sections 4.1 and 4.3). Layout trees remain temporary.

mod measurements;
mod stack;
pub(crate) use measurements::{FlowMeasurements, FlowNode};

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
    fn float_translation_preserves_unbreakable_margins() {
        let source = FlowGeometry {
            floats: vec![FloatGeometry {
                id: 7,
                top: 40.0,
                bottom: 60.0,
                margins: [40.0, 40.0],
            }],
            ..Default::default()
        };
        let mut geometry = FlowGeometry::default();
        geometry.append(&source, 15.0);
        let g = geometry.floats[0];
        assert_eq!((g.id, g.top, g.bottom), (7, 55.0, 75.0));
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
