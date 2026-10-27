//! KaminIDE patch: передача роста фрагментации после дорожек сетки в Taffy (css-grid-2 §12).
use super::{LayoutId, TaffyLayoutEngine};

impl TaffyLayoutEngine {
    /// Пространство, вставленное фрагментацией после каждой дорожки физической оси `axis`;
    /// индексы — как у `grid_tracks` (логический порядок с ведущими неявными дорожками).
    pub fn set_grid_fragmentation_growth(&mut self, id: LayoutId, axis: crate::Axis, growth: &[f32], scale: f32) -> bool {
        let axis = match axis {
            crate::Axis::Horizontal => taffy::geometry::AbsoluteAxis::Horizontal,
            crate::Axis::Vertical => taffy::geometry::AbsoluteAxis::Vertical,
        };
        let growth = growth.iter().map(|v| v * scale).collect();
        self.taffy.set_grid_fragmentation_growth(id.into(), axis, growth).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::prelude::*;

    #[test]
    fn growth_is_scaled_and_moves_the_following_row() {
        let mut engine = TaffyLayoutEngine::new();
        let scale = 2.0;
        let child = |engine: &mut TaffyLayoutEngine, row: i16| {
            engine.taffy.new_leaf(Style {
                size: Size { width: auto(), height: length(20.0) },
                grid_row: Line { start: line(row), end: line(row + 1) },
                ..Style::DEFAULT
            })
        };
        let first = child(&mut engine, 1).unwrap();
        let second = child(&mut engine, 2).unwrap();
        let root = engine
            .taffy
            .new_with_children(
                Style {
                    display: Display::Grid,
                    grid_template_rows: vec![auto(), auto()],
                    grid_template_columns: vec![length(20.0)],
                    ..Style::DEFAULT
                },
                &[first, second],
            )
            .unwrap();
        assert!(engine.set_grid_fragmentation_growth(root.into(), crate::Axis::Vertical, &[5.0], scale));
        engine.taffy.compute_layout(root, taffy::Size::MAX_CONTENT).unwrap();
        assert_eq!(engine.taffy.layout(second).unwrap().location.y, 30.0);
        assert!(!engine.set_grid_fragmentation_growth(root.into(), crate::Axis::Vertical, &[-1.0], scale));
    }
}
