//! Source-order painting of positioned descendants published by selected floats.

use super::{cursors, quads, top};

#[test]
fn float_selected_tree_prepares_hoisted_source_before_prepaint() {
    let html = "<!DOCTYPE html><style> aside { float: left; width: 100%; } \
        aside:before { content: ''; left: 50%; position: fixed; top: 60px; } \
        aside .wrapper { position: relative; } </style>\
        <aside><div class=wrapper></div></aside>";
    quads(html, 320., 200.);
}

#[test]
fn hoisted_float_child_keeps_geometry_and_source_paint_order() {
    for position in ["absolute", "fixed"] {
        for float in ["none", "left"] {
            for sibling in ["static", "relative"] {
                for top_value in ["auto", "60px"] {
                    let html = format!(
                        "<style>html,body{{margin:0}}aside{{float:{float};width:100%}}\
                         aside:before{{content:'';position:{position};left:50%;\
                         top:{top_value};width:8px;height:8px;background:red}}\
                         .wrapper{{position:{sibling};height:8px;background:#ddd}}\
                         </style><aside><div class=wrapper></div></aside>"
                    );
                    let y = if top_value == "auto" { 4. } else { 64. };
                    let expected = if top_value == "auto" && sibling == "relative" {
                        0xdddddd
                    } else {
                        0xff0000
                    };
                    let found = quads(&html, 320., 200.);
                    assert_eq!(
                        top(&found, 164., y).unwrap().to_rgb(),
                        gpui::rgb(expected),
                        "{position}, float={float}, sibling={sibling}, top={top_value}"
                    );
                }
            }
        }
    }
}

#[test]
fn hoisted_source_prepaint_preserves_hit_order_under_later_sibling() {
    let html = "<style>html,body{margin:0}aside{float:left;width:100%}\
        aside:before{content:'';position:fixed;left:50%;top:0;\
        width:8px;height:8px;background:red;cursor:pointer}\
        .wrapper{position:relative;height:8px;background:#ddd;cursor:crosshair}\
        </style><aside><div class=wrapper></div></aside>";
    let found = quads(html, 320., 200.);
    assert_eq!(top(&found, 164., 4.).unwrap().to_rgb(), gpui::rgb(0xdddddd));
    assert_eq!(
        cursors(html, false, 320., 200., &[(164., 4.)]),
        [gpui::CursorStyle::Crosshair]
    );
}

#[test]
fn selected_float_uses_the_receiving_containing_block_geometry() {
    for position in ["absolute", "fixed"] {
        for (inset, y) in [("top:60px", 81.), ("bottom:25%", 125.)] {
            let html = format!(
                "<style>html,body{{margin:0}}main{{position:relative;transform:translate(0);\
                 margin:17px 0 0 23px;width:200px;height:150px}}\
                 aside{{float:left;width:100%}}aside:before{{content:'';\
                 position:{position};left:50%;{inset};width:8px;height:8px;background:red}}\
                 .wrapper{{position:relative;height:8px;background:#ddd}}\
                 </style><main><aside><div class=wrapper></div></aside></main>"
            );
            let found = quads(&html, 320., 200.);
            assert_eq!(
                top(&found, 127., y).map(|color| color.to_rgb()),
                Some(gpui::rgb(0xff0000)),
                "{position}, {inset}"
            );
        }
    }
}

#[test]
fn nested_selected_floats_prepare_against_the_live_host() {
    let html = "<style>html,body{margin:0}section,aside{float:left;width:100%}\
        aside:before{content:'';position:fixed;left:50%;top:60px;\
        width:8px;height:8px;background:red}\
        .wrapper{position:relative;height:8px;background:#ddd}\
        </style><section><aside><div class=wrapper></div></aside></section>";
    let found = quads(html, 320., 200.);
    assert_eq!(
        top(&found, 164., 64.).map(|color| color.to_rgb()),
        Some(gpui::rgb(0xff0000))
    );
}
