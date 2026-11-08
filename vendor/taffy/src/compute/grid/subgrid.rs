//! KaminIDE patch: ПОДСЕТКА (css-grid-2 §9 `#subgrids`) на общем пути сетки.
//!
//! Спека — `refs/csswg-drafts/css-grid-2/Overview.bs:3555-3925`; референс —
//! Blink `third_party/blink/renderer/core/layout/grid/` (`grid_sizing_tree.cc`,
//! `grid_track_collection.cc` `CreateSubgridTrackCollection`,
//! `grid_layout_algorithm.cc` `ComputeSubgridIntrinsicSize`).
//!
//! Устройство (упрощённое дерево размеров Blink):
//! * Родительская сетка после размещения своих элементов «сплющивает» в себя
//!   элементы каждой подсетки-ребёнка (§9 (h) `#subgrid-item-contribution`):
//!   они размещаются ТЕМ ЖЕ кодом, что у самой подсетки (`place_items`), и
//!   вкладывают свои размеры в дорожки родителя по подсеточным осям; края
//!   подсетки и половина разницы зазоров идут им «extra margin»
//!   (§subgrid-margins, §subgrid-gaps), накапливаясь через уровни. Сама
//!   подсетка в подсеточной оси считается пустой (§9 (g)).
//! * Перед каждым замером и перед раскладкой подсетки родитель пишет ей
//!   РАЗРЕШЁННЫЕ размеры дорожек её пролёта (`publish_subgrid_tracks`,
//!   §9 (a) `#subgrid-tracks`). Подсетка с записью берёт их вместо своего
//!   шаблона: явных дорожек ровно по пролёту (§9 (b)), неявных в подсеточной
//!   оси нет — область элемента зажимается (§9 (f) `#subgrid-implicit`),
//!   выравнивание содержимого в этой оси не действует (§subgrid-grid-alignment).
#[path = "fixed_standalone_tracks.rs"]
mod fixed_standalone_tracks;
use fixed_standalone_tracks::standalone_tracks_with_known;
#[path = "nested_standalone_size.rs"]
mod nested_standalone_size;
use super::types::{CellOccupancyMatrix, GridItem, NamedLineResolver, TrackCounts};
use super::OriginZeroLine;
use crate::geometry::{Line, Size};
use crate::tree::{LayoutPartialTreeExt, NodeId};
use crate::util::sys::Vec;
use crate::util::{MaybeResolve, ResolveOrZero};
use crate::GenericRepetition;

/// Результат размещения элементов сетки (шаги 2-4 `compute_grid_layout`).
pub(super) struct PlacedGrid<S: crate::CheapCloneStr> {
    /// Элементы в потоке.
    pub items: Vec<GridItem>,
    /// Занятость клеток (для схлопывания `auto-fit`).
    pub cell_occupancy_matrix: CellOccupancyMatrix,
    /// Имена линий.
    pub name_resolver: NamedLineResolver<S>,
    /// Число дорожек колонок.
    pub col_counts: TrackCounts,
    /// Число дорожек рядов.
    pub row_counts: TrackCounts,
    /// Native auto-repeat counts used by track initialization and detailed names.
    pub col_auto_repetition_count: u16,
    /// KaminIDE patch: Resolved number of automatic row repetitions.
    pub row_auto_repetition_count: u16,
}

/// Край подсетки в дорожках корневой сетки: накопленный «extra margin» у
/// первой и последней дорожки её пролёта (Blink `StartExtraMargin`/
/// `EndExtraMargin`, `grid_track_collection.cc:558-569`).
pub(super) struct SubgridEdge {
    /// Ось: `true` — колонки.
    columns: bool,
    /// Пролёт подсетки в координатах корневой сетки.
    span: Line<OriginZeroLine>,
    /// Накопленный край у начала пролёта.
    start: f32,
    /// Накопленный край у конца пролёта.
    end: f32,
    /// Разница зазора подсетки и зазора корневой сетки (накопленная,
    /// Blink `AccumulatedGutterSizeDelta`).
    delta: f32,
    /// Подсетка-ребёнок корневого контейнера, под которой лежит эта.
    root: NodeId,
}

#[path = "subgrid_placement.rs"]
mod placement;
pub(super) use placement::{bounded_subgrid_count, place_items};
#[path = "subgrid_track_api.rs"]
mod track_api;
pub(super) use track_api::{fixed_track_sizes, initialize_subgrid_tracks, own_subgrid_tracks};
#[path = "subgrid_edges.rs"]
mod edges;
pub(super) use edges::{linked_axes, subgrid_edges, subgrid_gaps};
#[path = "subgrid_flatten_entry.rs"]
mod flatten_entry;
pub(super) use flatten_entry::flatten_subgrid_items_async;
#[path = "subgrid_flatten_recursive.rs"]
mod flatten_recursive;
pub(super) use flatten_recursive::flatten_into_async;
#[path = "subgrid_floors.rs"]
mod floors;
pub(super) use floors::{apply_lanes_auto_floors, apply_subgrid_floors, partition_for_axis};
#[path = "subgrid_publish.rs"]
mod publish;
pub(super) use publish::{lanes_subgridded, publish_lanes_subgrid, publish_subgrid_tracks};
#[path = "subgrid_setup.rs"]
mod setup;

#[path = "subgrid_cross_constraints.rs"]
mod cross_constraints;
#[path = "subgrid_stack_constraint.rs"]
mod stack_constraint;

#[path = "subgrid_item_edges.rs"]
mod item_edges;

#[cfg(test)]
mod kamin_placement_tests {
    use super::super::MAX_GRID_TRACKS;
    use super::placement::contributes_to_placement;
    use super::*;
    use crate::style::{Display, Position};
    type Style = crate::style::Style;

    #[test]
    fn absolute_and_hidden_styles_do_not_contribute_to_placement() {
        let mut style = Style::DEFAULT;
        style.position = Position::Absolute;
        assert!(!contributes_to_placement(&style));
        style.position = Position::Relative;
        style.display = Display::None;
        assert!(!contributes_to_placement(&style));
    }

    #[test]
    fn visible_inflow_style_contributes_to_placement() {
        assert!(contributes_to_placement(&Style::DEFAULT));
    }

    #[test]
    fn oversized_inherited_span_keeps_positive_signed_coordinates() {
        let count = bounded_subgrid_count(u16::MAX);
        assert_eq!(count, MAX_GRID_TRACKS);
        assert!(count as i16 > 0);
        assert_eq!(bounded_subgrid_count(0), 1);
        assert_eq!(bounded_subgrid_count(12), 12);
    }
}

#[cfg(all(test, feature = "taffy_tree"))]
mod depth_tests {
    use crate::prelude::*;
    use crate::{SUBGRID_COLUMNS, SUBGRID_ROWS};

    fn intrinsic_height(depth: usize) -> f32 {
        let mut tree = TaffyTree::<()>::new();
        tree.disable_rounding();
        let mut child = tree
            .new_leaf(Style {
                size: Size {
                    width: length(20.0),
                    height: length(77.0),
                },
                ..Style::DEFAULT
            })
            .unwrap();
        for _ in 0..depth {
            child = tree
                .new_with_children(
                    Style {
                        display: Display::Grid,
                        subgrid: SUBGRID_COLUMNS | SUBGRID_ROWS,
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
        }
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: Size {
                        width: length(100.0),
                        height: Dimension::AUTO,
                    },
                    grid_template_columns: vec![auto()],
                    grid_template_rows: vec![auto()],
                    ..Style::DEFAULT
                },
                &[child],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        tree.layout(root).unwrap().size.height
    }

    #[test]
    fn deeply_nested_subgrids_preserve_intrinsic_contributions_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                // Both sides of the former cutoff, plus depth beyond a u8 counter.
                for depth in [12, 13, 32, 256] {
                    assert_eq!(intrinsic_height(depth), 77.0, "subgrid depth {depth}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
