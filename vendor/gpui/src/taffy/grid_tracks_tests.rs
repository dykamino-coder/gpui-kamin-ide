//! Дорожки пустой сетки доступны без фиктивного ребёнка и только после раскладки.
use super::{TaffyLayoutEngine, ToTaffy};
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

#[test]
fn board_appearance_grid_calc_uses_each_axis_and_scales_only_pixels() {
    use crate::{Display as GDisplay, GridTrack as G, Style as GStyle, px, size};
    for scale in [1.0, 1.25, 2.0] {
        let mut engine = TaffyLayoutEngine::new();
        let _scope = engine.taffy.calc_scope();
        let tracks = vec![
            G::Calc {
                fraction: 0.25,
                add: px(10.0),
            },
            G::MinMax(Box::new((
                G::Pixels(px(0.0)),
                G::Calc {
                    fraction: 0.25,
                    add: px(-10.0),
                },
            ))),
        ];
        let native: Style = GStyle {
            display: GDisplay::Grid,
            size: size(px(400.0).into(), px(200.0).into()),
            grid_template_cols: Some(tracks.clone()),
            grid_template_rows: Some(tracks),
            ..GStyle::default()
        }
        .to_taffy(px(16.0), scale);
        let root: LayoutId = engine.taffy.new_leaf(native).unwrap().into();
        engine
            .taffy
            .compute_layout(root.into(), Size::MAX_CONTENT)
            .unwrap();
        let (_, columns, rows) = engine.grid_tracks(root, scale).unwrap();
        assert_eq!(columns, vec![(0.0, 110.0), (110.0, 200.0)]);
        assert_eq!(rows, vec![(0.0, 60.0), (60.0, 100.0)]);
    }
}

#[test]
fn board_appearance_grid_fit_calc_retains_an_intrinsic_limit() {
    use crate::{GridTrack as G, Style as GStyle, px};
    for scale in [1.0, 1.25] {
        let engine = TaffyLayoutEngine::new();
        let _scope = engine.taffy.calc_scope();
        let native: Style = GStyle {
            grid_template_cols: Some(vec![G::FitContentCalc {
                fraction: 0.25,
                add: px(10.0),
            }]),
            ..GStyle::default()
        }
        .to_taffy(px(16.0), scale);
        let GridTemplateComponent::Single(track) = native.grid_template_columns[0] else {
            panic!()
        };
        assert!(track.min.is_auto());
        assert!(track.max.is_fit_content());
        // Layout resolves the registered expression against its own axis.
        let resolver = |handle, basis| taffy::tree::calc_value(handle, basis);
        assert_eq!(track.max.fit_content_limit(None, resolver), None);
        assert_eq!(
            track.max.fit_content_limit(Some(400.0 * scale), resolver),
            Some(110.0 * scale)
        );
    }
}

#[test]
fn board_appearance_grid_auto_repeat_resolves_calc_for_each_axis() {
    use crate::{Display as GDisplay, GridTrack as G, Style as GStyle, px, size};
    for scale in [1.0, 1.25] {
        let mut engine = TaffyLayoutEngine::new();
        let _scope = engine.taffy.calc_scope();
        let tracks = vec![G::AutoRepeat {
            fit: false,
            tracks: vec![G::MinMax(Box::new((
                G::Calc {
                    fraction: 0.25,
                    add: px(-10.0),
                },
                G::Fraction(1.0),
            )))],
        }];
        let native: Style = GStyle {
            display: GDisplay::Grid,
            size: size(px(400.0).into(), px(200.0).into()),
            grid_template_cols: Some(tracks.clone()),
            grid_template_rows: Some(tracks),
            ..GStyle::default()
        }
        .to_taffy(px(16.0), scale);
        let root: LayoutId = engine.taffy.new_leaf(native).unwrap().into();
        engine
            .taffy
            .compute_layout(root.into(), Size::MAX_CONTENT)
            .unwrap();
        let (_, columns, rows) = engine.grid_tracks(root, scale).unwrap();
        assert_eq!(
            columns,
            vec![(0.0, 100.0), (100.0, 200.0), (200.0, 300.0), (300.0, 400.0)]
        );
        assert_eq!(
            rows,
            vec![
                (0.0, 40.0),
                (40.0, 80.0),
                (80.0, 120.0),
                (120.0, 160.0),
                (160.0, 200.0)
            ]
        );
    }
}
