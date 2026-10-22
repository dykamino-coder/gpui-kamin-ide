//! Дорожки пустой сетки доступны без фиктивного ребёнка и только после раскладки.
use super::TaffyLayoutEngine;
use crate::{LayoutId, Pixels, point};
use taffy::prelude::*;

#[test]
fn empty_grid_tracks_preserve_native_sizes_origin_and_scale() {
    for scale in [1.0, 1.25, 2.0] {
        let mut engine = TaffyLayoutEngine::new();
        let root: LayoutId = engine
            .taffy
            .new_leaf(Style {
                display: Display::Grid,
                grid_template_rows: vec![length(20.0 * scale), length(30.0 * scale)],
                grid_template_columns: vec![length(40.0 * scale)],
                ..Style::DEFAULT
            })
            .unwrap()
            .into();
        assert!(engine.grid_tracks(root, scale).is_none());
        engine
            .taffy
            .compute_layout(root.into(), Size::MAX_CONTENT)
            .unwrap();
        engine.set_root_origin(root, point(Pixels(3.25), Pixels(7.5)), scale);
        let (origin, columns, rows) = engine.grid_tracks(root, scale).unwrap();
        assert_eq!(origin, (3.25, 7.5));
        assert_eq!(columns, vec![(0.0, 40.0)]);
        assert_eq!(rows, vec![(0.0, 20.0), (20.0, 50.0)]);
        assert!(engine.parent_grid_tracks(root, scale).is_none());
        engine.taffy.mark_dirty(root.into()).unwrap();
        assert!(engine.grid_tracks(root, scale).is_none());
    }
}
