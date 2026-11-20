//! A wheel moves only the axes a box scrolls, as in browsers: a vertical
//! wheel over a box that scrolls only horizontally scrolls the root, and the
//! box stays where it is.

use super::super::fixed_tests::root;
use super::*;

#[gpui::test]
fn a_vertical_wheel_over_a_horizontal_scroller_scrolls_the_root(cx: &mut TestAppContext) {
    let html = format!(
        "{STYLE}<div id=wide style='overflow-x:scroll;height:100px'>\
         <div style='width:3000px;height:80px;background:#ccc'></div></div>{}",
        rows(400)
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for step in 1..=3 {
        wheel_at(vcx, 100., 40., -60.);
        frame(vcx);
        let offset = root(&view, vcx).offset();
        assert_eq!(offset, point(px(0.), px(-60. * step as f32)), "step {step}");
    }
}
