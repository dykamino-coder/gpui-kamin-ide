//! Text breaks have different CSSOM box behavior from ordinary inline boxes.

use super::*;

#[gpui::test]
fn geometry_line_break_keeps_its_zero_width_fragment(cx: &mut TestAppContext) {
    let elements = snapshot(
        cx,
        "<style>body{margin:0;font:20px/30px monospace}br{padding:7px;border:3px solid}</style><span id='before'>ab</span><br id='break'><span id='after'>cd</span><wbr id='opportunity'><br id='contents' style='display:contents'><br id='hidden' style='display:none'>",
        300.,
    );
    let before = find(&elements, "before");
    let br = find(&elements, "break");
    let after = find(&elements, "after");
    assert_eq!(br.client_rects.len(), 1, "{:?}", br.client_rects);
    assert_eq!(br.border_box.width, 0.);
    assert!(br.border_box.height > 0.);
    assert_eq!(
        br.border_box.x,
        before.border_box.x + before.border_box.width
    );
    assert_eq!(br.border_box.y, before.border_box.y);
    assert_eq!(br.border_box.height, before.border_box.height);
    assert_eq!(after.border_box.y - before.border_box.y, 30.);
    assert_eq!(br.padding_boxes, br.client_rects);
    assert_eq!(br.content_boxes, br.client_rects);
    for id in ["opportunity", "contents", "hidden"] {
        assert!(find(&elements, id).client_rects.is_empty(), "{id}");
    }
}
