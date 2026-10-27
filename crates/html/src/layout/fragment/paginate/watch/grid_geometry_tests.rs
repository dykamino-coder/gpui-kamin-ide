//! Сетки рекордера: ряды элементов — из финального размещения решателя, а рост после дорожки,
//! записанный фрагментацией, двигает следующие ряды, не меняя размеров уже уложенных.
use super::Watched;
use crate::layout::fragment::flow::blocks::{BlockBox, GridBox, Laid};
use crate::layout::fragment::geometry::Node;
use crate::layout::fragment::types::StackAxis;
use gpui::{
    AlignItems, AvailableSpace, Display, GridLocation, GridPlacement, GridTrack, Style, point, px,
    size,
};

const GRID: u64 = 10;

/// Сетка `GRID` с элементами `1..=3`, как её собирает стопка копии.
fn recorder(axis: StackAxis) -> std::rc::Rc<Watched> {
    let doc = crate::dom::parse(
        "<div style='display:grid'><div></div><div></div><div></div></div>",
        "",
    );
    fn div(n: &crate::dom::Node) -> Option<&crate::dom::Element> {
        let crate::dom::Node::Element(e) = n else {
            return None;
        };
        if e.tag == "div" {
            return Some(e);
        }
        e.children.iter().find_map(div)
    }
    let root = doc.iter().find_map(div).expect("grid");
    let mut grid: GridBox = BlockBox::collect(
        root,
        crate::layout::fragment::ShapeCx::COLUMNS,
        doc.values(),
    )
    .grid
    .expect("grid items");
    for (id, item) in (1..).zip(grid.items.iter_mut()) {
        item.id = id;
    }
    let blocks = BlockBox {
        id: GRID,
        grid: Some(grid),
        ..Default::default()
    };
    Watched::for_blocks_axis(blocks, axis, false)
}

/// Элемент `1` охватывает два ряда, но прижат к концу области; `2` — во втором ряду, `3` — в
/// третьем по отрицательным линиям. Ряды идут по блочной оси `axis`.
fn layout(cx: &mut gpui::VisualTestContext, rec: &Watched, axis: StackAxis) -> Laid {
    let vertical = axis.is_vertical();
    let place = |start: i16, end: GridPlacement| {
        let lines = GridPlacement::Line(start)..end;
        Some(if vertical {
            GridLocation {
                column: lines,
                ..Default::default()
            }
        } else {
            GridLocation {
                row: lines,
                ..Default::default()
            }
        })
    };
    let extent = |length: f32| {
        if vertical {
            size(px(length).into(), px(10.).into())
        } else {
            size(px(10.).into(), px(length).into())
        }
    };
    let mut out = None;
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let styles = [
                Style {
                    size: extent(40.),
                    align_self: Some(AlignItems::End),
                    justify_self: Some(AlignItems::End),
                    grid_location: place(1, GridPlacement::Span(2)),
                    ..Style::default()
                },
                Style {
                    size: extent(20.),
                    grid_location: place(2, GridPlacement::Line(3)),
                    ..Style::default()
                },
                Style {
                    grid_location: place(-2, GridPlacement::Line(-1)),
                    ..Style::default()
                },
            ];
            let ids: Vec<_> = styles
                .into_iter()
                .map(|s| window.request_layout(s, [], app))
                .collect();
            let tracks = vec![GridTrack::Pixels(px(50.)); 3];
            let grid = Style {
                display: Display::Grid,
                grid_template_rows: (!vertical).then(|| tracks.clone()),
                grid_template_cols: vertical.then_some(tracks),
                ..Style::default()
            };
            let root = window.request_layout(grid, ids.clone(), app);
            if let Some(growth) = rec.grid_growth.borrow().get(&GRID) {
                window.set_grid_fragmentation_growth(root, super::axis::solver_axis(axis), growth);
            }
            for (id, lid) in (1..).zip(&ids) {
                rec.seen
                    .borrow_mut()
                    .insert(id, Node::capture(*lid, window));
            }
            rec.seen
                .borrow_mut()
                .insert(GRID, Node::capture(root, window));
            let space = AvailableSpace::Definite(px(400.));
            window.compute_layout(root, size(space, space), app);
            out = Some(rec.laid_axis(
                point(px(0.), px(0.)),
                size(px(400.), px(400.)),
                axis,
                window,
            ));
            gpui::div()
        },
    );
    out.unwrap()
}

#[gpui::test]
fn item_rows_follow_the_solver_placement_on_the_block_axis(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for axis in [StackAxis::Horizontal, StackAxis::VerticalLr] {
        let rec = recorder(axis);
        let laid = layout(cx, &rec, axis);
        assert_eq!(
            laid.rows(GRID).unwrap(),
            [(0., 50.), (50., 100.), (100., 150.)]
        );
        // The box of `1` lies in the second row only; its area spans the first two.
        assert_eq!(laid.at(1), Some((60., 100.)), "{axis:?}");
        assert_eq!(laid.item_rows(1), Some((0, 1)), "{axis:?}");
        assert_eq!(laid.item_rows(2), Some((1, 1)));
        assert_eq!(laid.item_rows(3), Some((2, 2)));
    }
}

#[gpui::test]
fn row_growth_moves_following_rows_and_keeps_completed_items(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    for axis in [StackAxis::Horizontal, StackAxis::VerticalLr] {
        let rec = recorder(axis);
        assert!(rec.grow_grid_row(GRID, 0, 30.));
        assert!(!rec.grow_grid_row(GRID, 1, 0.));
        let laid = layout(cx, &rec, axis);
        assert_eq!(
            laid.rows(GRID).unwrap(),
            [(0., 50.), (80., 130.), (130., 180.)]
        );
        // The spanning area grows with the pushed row; a fixed-size item keeps its size.
        assert_eq!(laid.at(1), Some((90., 130.)), "{axis:?}");
        assert_eq!(laid.at(2), Some((80., 100.)));
        assert_eq!(laid.at(GRID), Some((0., 180.)));
        assert_eq!(laid.item_rows(1), Some((0, 1)));
    }
}

/// Ряд сетки по линиям `row..row + 1` с заданной высотой.
fn leaf(window: &mut gpui::Window, app: &mut gpui::App, height: f32, row: i16) -> gpui::LayoutId {
    let location = GridLocation {
        row: GridPlacement::Line(row)..GridPlacement::Line(row + 1),
        ..Default::default()
    };
    window.request_layout(
        Style {
            size: size(px(10.).into(), px(height).into()),
            grid_location: Some(location),
            ..Style::default()
        },
        [],
        app,
    )
}

#[gpui::test]
fn parent_growth_reaches_subgrid_rows_and_spares_nested_alignment(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    let auto_rows = || Some(vec![GridTrack::Auto, GridTrack::Auto]);
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let space = size(AvailableSpace::MaxContent, AvailableSpace::MaxContent);
            // Подсетка по рядам: рост родителя после первого ряда двигает её второй ряд.
            let kids = [leaf(window, app, 50., 1), leaf(window, app, 40., 2)];
            let subgrid = Style {
                display: Display::Grid,
                grid_subgrid: 2,
                grid_location: Some(GridLocation {
                    row: GridPlacement::Line(1)..GridPlacement::Line(3),
                    ..Default::default()
                }),
                ..Style::default()
            };
            let sub = window.request_layout(subgrid, kids, app);
            let grid = Style {
                display: Display::Grid,
                grid_template_rows: auto_rows(),
                ..Style::default()
            };
            let root = window.request_layout(grid.clone(), [sub], app);
            window.set_grid_fragmentation_growth(sub, gpui::Axis::Vertical, &[30.]);
            window.set_grid_fragmentation_growth(root, gpui::Axis::Vertical, &[30.]);
            window.compute_layout(root, space, app);
            let top = window.layout_bounds(sub).origin.y;
            assert_eq!(window.layout_bounds(kids[1]).origin.y - top, px(80.));
            assert_eq!(window.layout_bounds(sub).size.height, px(120.));
            // Вложенная сетка растянута на ряд, измеренный вместе с её ростом: `normal`
            // выравнивание не раздаёт этот рост её рядам.
            let kids = [leaf(window, app, 50., 1), leaf(window, app, 40., 2)];
            let nested = window.request_layout(grid.clone(), kids, app);
            let outer = Style {
                grid_template_rows: Some(vec![GridTrack::Auto]),
                ..grid
            };
            let root = window.request_layout(outer, [nested], app);
            window.set_grid_fragmentation_growth(nested, gpui::Axis::Vertical, &[30.]);
            window.compute_layout(root, space, app);
            let top = window.layout_bounds(nested).origin.y;
            assert_eq!(window.layout_bounds(nested).size.height, px(120.));
            assert_eq!(window.layout_bounds(kids[1]).origin.y - top, px(80.));
            gpui::div()
        },
    );
}
