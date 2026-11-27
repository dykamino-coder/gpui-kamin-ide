//! `vertical-align` on `::first-line` shifts neither the line nor the block.

use super::quads;

#[test]
fn first_line_vertical_align_changes_nothing() {
    // The root inline box of the line nests inside `::first-line`
    // (css-pseudo-4 §first-text-line): there is nothing to align against.
    let page = |rule: &str| {
        format!(
            "<style>body,p{{margin:0}} p{{width:120px;font:20px/30px monospace;\
             background:yellow}} p::first-line{{background:lime;{rule}}} \
             span{{display:inline-block;width:20px;height:10px;background:red}} \
             div{{height:10px;background:blue}}</style>\
             <p>one <span></span> two three four five six seven</p><div></div>"
        )
    };
    let plain = quads(&page(""), 200., 300.);
    let blue = plain
        .iter()
        .find(|(_, c)| c.to_rgb() == gpui::rgb(0x0000ff))
        .expect("block after the paragraph");
    assert!(f32::from(blue.0.origin.y) > 0.);
    for rule in [
        "vertical-align:20px",
        "vertical-align:super",
        "vertical-align:top",
        "vertical-align:-10px",
    ] {
        assert_eq!(quads(&page(rule), 200., 300.), plain, "{rule}");
    }
}
