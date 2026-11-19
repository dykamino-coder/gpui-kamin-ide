//! Positioned list items retain their containing block and stay outside flow.

use super::rendered::{quads, top};

#[test]
fn positioned_list_items_keep_css_position_for_every_marker_mode() {
    for position in ["absolute", "fixed"] {
        for marker in ["none outside", "disc outside", "disc inside"] {
            // A transform makes the nav the fixed containing block as well.
            let containing = if position == "fixed" {
                "transform:translateX(0);"
            } else {
                ""
            };
            let html = format!(
                "<style>html,body{{margin:0}}nav{{position:absolute;right:24px;\
                 top:40px;width:400px;height:160px;{containing}background:#ddd}}\
                 ul{{margin:0;padding:0}}li{{position:{position};top:40px;right:0;\
                 width:70px;height:70px;z-index:1;list-style:{marker};background:red}}\
                 .flow{{height:10px;background:lime}}</style>\
                 <nav><ul><li></li></ul><div class=flow></div></nav>"
            );
            let found = quads(&html, 800., 600.);
            let (left, y) = (706., 80.);
            assert_eq!(
                top(&found, left + 5., y + 5.),
                Some(gpui::rgb(0xff0000).into()),
                "{position}/{marker}: {found:?}"
            );
            assert_eq!(
                top(&found, 380., 45.),
                Some(gpui::rgb(0x00ff00).into()),
                "flow: {position}/{marker}"
            );
            assert_ne!(
                top(&found, left - 2., y + 5.),
                Some(gpui::rgb(0xff0000).into()),
                "left edge: {position}/{marker}"
            );
        }
    }
}

#[test]
fn absolute_list_item_shrink_to_fit_keeps_its_right_inset() {
    let html = "<style>html,body{margin:0}nav{position:absolute;right:24px;\
        top:40px;width:400px}li{position:absolute;right:0;list-style:none}\
        a{display:block;width:70px;height:70px}\
        a::before{content:'';position:absolute;left:0;width:70px;height:70px;\
        background:red}</style><nav><li><a></a></li></nav>";
    let found = quads(html, 800., 600.);
    assert_eq!(
        top(&found, 710., 45.).map(|color| color.to_rgb()),
        Some(gpui::rgb(0xff0000))
    );
    assert_ne!(
        top(&found, 380., 45.).map(|color| color.to_rgb()),
        Some(gpui::rgb(0xff0000))
    );
}
