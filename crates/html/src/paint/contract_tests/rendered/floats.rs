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

#[test]
fn nested_normal_blocks_share_the_outer_float_exclusion() {
    for depth in [0, 1, 2, 3, 8, 16, 64] {
        let inner = format!(
            "{}<p><span></span><span></span><span></span><span></span></p>{}",
            "<div>".repeat(depth),
            "</div>".repeat(depth),
        );
        let html = format!(
            "<style>body,p{{margin:0}} .float{{float:right;width:100px;height:100px;\
             background:blue}} span{{display:inline-block;width:60px;height:20px;\
             vertical-align:top;background:red}} p{{font-size:0;line-height:20px}}</style>\
             <div class=float></div>{inner}"
        );
        let found = quads(&html, 200., 200.);
        let red: Vec<_> = found
            .iter()
            .filter(|(_, c)| c.to_rgb() == gpui::rgb(0xff0000))
            .map(|(b, _)| (f32::from(b.origin.x), f32::from(b.origin.y)))
            .collect();
        assert_eq!(
            red,
            vec![(0., 0.), (0., 20.), (0., 40.), (0., 60.)],
            "depth {depth}"
        );
    }
}

#[test]
fn nested_independent_context_keeps_its_own_lines_below_the_outer_float() {
    let html = "<style>body,p{margin:0}.float{float:right;width:100px;height:100px;\
                background:blue}.isolated{display:flow-root;width:200px}\
                span{display:inline-block;width:60px;height:20px;vertical-align:top;\
                background:red}p{font-size:0;line-height:20px}</style>\
                <div class=float></div><div><div class=isolated><p>\
                <span></span><span></span><span></span><span></span></p></div></div>";
    let found = quads(html, 200., 200.);
    let mut red: Vec<_> = found
        .iter()
        .filter(|(_, c)| c.to_rgb() == gpui::rgb(0xff0000))
        .map(|(b, _)| (f32::from(b.origin.y), f32::from(b.origin.x)))
        .collect();
    red.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    assert_eq!(red, vec![(100., 0.), (100., 60.), (100., 120.), (120., 0.)]);
}
