//! Смены `class`, чей перерасчёт стиля меняет только цвета рамок коробки
//! (`revision::repaint`, Blink `border-visual`): блочная коробка вне таблиц
//! перекрашивается на месте, а сросшиеся рамки таблиц, стороны строчных
//! коробок и толщина рамки собираются заново.

use super::update;

fn page(style: &str) -> String {
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;color:#000;background:#fff}}\
         .box,td,span{{border:2px solid #f00}}{style}</style></head><body><div class=page>\
         <div class=box><p>a <span>b</span></p></div>\
         <table style=\"border-collapse:collapse\"><tr><td>1</td><td>2</td></tr></table>\
         </div></body></html>"
    )
}

/// Обе стороны правки `class=page` → `class="page lit"`.
fn both(style: &str) -> (bool, bool) {
    let old = page(style);
    let new = old.replace("class=page", "class=\"page lit\"");
    (update(&old, &new, ""), update(&new, &old, ""))
}

#[test]
fn border_colours_of_a_block_are_restyled_in_place() {
    assert_eq!(both(".lit .box{border-color:#00f}"), (true, true));
    assert_eq!(both(".lit .box{border-top-color:#0f0}"), (true, true));
    // A block without a border style keeps its colours as values only.
    assert_eq!(both(".lit p{border-color:red}"), (true, true));
    // With the colour of the text, which the subtree inherits.
    assert_eq!(
        both(".lit .box{border-color:#00f;color:#123}"),
        (true, true)
    );
}

#[test]
fn border_colours_of_table_parts_and_inline_boxes_rebuild_and_widths_rebox() {
    assert_eq!(both(".lit td{border-color:#00f}"), (false, false));
    assert_eq!(both(".lit span{border-color:#00f}"), (false, false));
    assert_eq!(both(".lit .box{border-width:3px}"), (true, true));
    assert_eq!(both(".lit .box{border-style:dashed}"), (true, true));
}
