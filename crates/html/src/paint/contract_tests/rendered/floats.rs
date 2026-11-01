//! Float foreground stays atomic even when its measured tree is retained.

use super::{quads, top};

#[test]
fn later_float_background_covers_the_earlier_floats_inline_foreground() {
    for overflow in ["visible", "scroll"] {
        let html = format!(
            "<style>body{{margin:0}} .first{{float:left;background:red;padding:20px;\
             box-sizing:border-box;width:100px;height:100px;overflow:{overflow}}} \
             .second{{float:left;width:100px;height:100px;background:green;margin-left:-100px}} \
             span{{display:inline-block;width:50px;height:50px}}</style>\
             <div class=first><div style='height:200px;background:yellow'>\
             <span style='background:blue'></span></div></div>\
             <div class=second><span style='background:magenta'></span></div>"
        );
        let found = quads(&html, 200., 200.);
        assert_eq!(top(&found, 60., 60.).unwrap().to_rgb(), gpui::rgb(0x008000));
        assert_eq!(top(&found, 25., 25.).unwrap().to_rgb(), gpui::rgb(0xff00ff));
    }
}
