//! Геометрия составных коробок и преобразованных якорей.

use super::*;

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.002, "{actual} != {expected}");
}

#[gpui::test]
fn geometry_transformed_anchor_keeps_the_positioned_box(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}#anchor{anchor-name:--target;width:40px;height:20px;transform:scale(2);transform-origin:0 0}#popup{position:absolute;position-anchor:--target;left:anchor(right);top:anchor(top);width:10px;height:10px}</style><div id=anchor></div><div id=popup></div>",
        300.,
    );
    let anchor = find(&elements, "anchor").border_box;
    near(anchor.width, 80.);
    near(anchor.height, 40.);
    let popup = find(&elements, "popup").border_box;
    near(popup.x, 80.);
    near(popup.y, 0.);
    near(popup.width, 10.);
    near(popup.height, 10.);
}

#[gpui::test]
fn geometry_fragmented_projective_box_maps_physical_column_corners(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}.cols{column-count:2;column-gap:20px;column-fill:auto;width:220px;height:100px}#box,#abs{width:100px;height:180px;transform-origin:0 0;transform:perspective(400px) rotateY(60deg)}#owner{position:relative;height:180px}#abs{position:absolute;top:0;left:0}</style><div class=cols><div id=box></div></div><div class=cols><div id=owner><div id=abs></div></div></div>",
        300.,
    );
    for (id, y) in [("box", 0.), ("abs", 100.)] {
        let rects = &find(&elements, id).client_rects;
        assert_eq!(rects.len(), 2, "{id}: {rects:?}");
        for (rect, (x, width, height)) in rects.iter().zip([
            (0., 41.1013069153, 100.),
            (47.6263198853, 26.8835754395, 63.5017585754),
        ]) {
            for (property, actual, expected) in [
                ("x", rect.x, x),
                ("y", rect.y, y),
                ("width", rect.width, width),
                ("height", rect.height, height),
            ] {
                assert!(
                    (actual - expected).abs() < 0.002,
                    "{id} {property}: {rects:?}, expected {expected}"
                );
            }
        }
    }
}

#[gpui::test]
fn geometry_table_row_transform_uses_all_cell_boxes(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}table{border-collapse:collapse;border-spacing:0}td{width:50px;height:40px;padding:0}tr{transform:scale(2)}#a,#b{width:50px;height:40px}</style><table><tr id=row><td><div id=a></div></td><td><div id=b></div></td></tr></table>",
        300.,
    );
    for (id, x, width) in [("row", -50., 200.), ("a", -50., 100.), ("b", 50., 100.)] {
        let element = find(&elements, id);
        assert_eq!(
            element.client_rects.len(),
            1,
            "{id}: {:?}",
            element.client_rects
        );
        let rect = element.border_box;
        near(rect.x, x);
        near(rect.y, -20.);
        near(rect.width, width);
        near(rect.height, 80.);
    }
}

#[gpui::test]
fn geometry_table_tracks_keep_rowspans_empty_rows_and_column_groups(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}table{border-collapse:collapse;border-spacing:0;table-layout:fixed;width:100px}td{padding:0}#r1{height:30px}#r2{height:40px}#empty{height:10px}#span{height:70px}</style><table><colgroup id=cg><col id=c1><col id=c2></colgroup><tbody id=group><tr id=r1><td id=span rowspan=2></td><td></td></tr><tr id=r2><td></td></tr><tr id=empty></tr></tbody></table>",
        360.,
    );
    for (id, x, y, width, height) in [
        ("cg", 0., 0., 100., 80.),
        ("c1", 0., 0., 50., 80.),
        ("c2", 50., 0., 50., 80.),
        ("group", 0., 0., 100., 80.),
        ("r1", 0., 0., 100., 30.),
        ("r2", 0., 30., 100., 40.),
        ("empty", 0., 70., 100., 10.),
    ] {
        let element = find(&elements, id);
        assert_eq!(
            element.client_rects.len(),
            1,
            "{id}: {:?}",
            element.client_rects
        );
        let rect = element.border_box;
        near(rect.x, x);
        near(rect.y, y);
        near(rect.width, width);
        near(rect.height, height);
    }
}

#[gpui::test]
fn geometry_zero_height_table_parts_keep_degenerate_rectangles(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0}table{border-collapse:collapse;border-spacing:0;width:100px}col{width:50px}</style><table id=t><colgroup id=cg><col id=c1><col id=c2></colgroup><tbody id=g></tbody></table><table id=u><tbody id=h><tr id=r></tr></tbody></table>",
        360.,
    );
    for (id, x, width) in [
        ("t", 0., 100.),
        ("cg", 0., 100.),
        ("c1", 0., 50.),
        ("c2", 50., 50.),
        ("g", 0., 100.),
        ("u", 0., 100.),
        ("h", 0., 100.),
        ("r", 0., 100.),
    ] {
        let element = find(&elements, id);
        assert_eq!(
            element.client_rects.len(),
            1,
            "{id}: {:?}",
            element.client_rects
        );
        let rect = element.border_box;
        near(rect.x, x);
        near(rect.y, 0.);
        near(rect.width, width);
        near(rect.height, 0.);
    }
}
