//! A stack whose width the layout decides (in a float) lays its columns out
//! once per frame: prepaint takes the columns of the final measure.
//! Counted with `crate::work`; run with `--features work-counters`.

use crate::layout::multicol::work_frame::{frame_work, total};
use crate::work::Counter;

fn page(items: usize) -> String {
    let item = "<p style='margin:0 0 3px'>One two three four five six seven eight.</p>";
    format!(
        "<style>body{{margin:0;font:16px Arial}}</style>\
         <div style='float:left'><div style='columns:3 150px;column-gap:10px'>{}</div></div>",
        item.repeat(items)
    )
}

/// Child layouts and kept measures of one frame.
fn frame(cx: &mut gpui::TestAppContext, html: &str) -> (u64, u64) {
    let work = frame_work(cx, html);
    (
        total(&work, Counter::MulticolChildLayouts),
        total(&work, Counter::MulticolMeasureReuses),
    )
}

#[gpui::test]
fn a_shrink_to_fit_stack_prepares_its_columns_once_per_width(cx: &mut gpui::TestAppContext) {
    let items = 12;
    let (layouts, reuses) = frame(cx, &page(items));
    // The float asks the stack at its width three times, then prepaint: one
    // preparation, two repeated answers and the columns taken by prepaint.
    assert!(
        reuses >= 3,
        "{reuses} reuses: the width's columns were prepared again"
    );
    // Two intrinsic queries prepare at no width, one measure at the width,
    // none in prepaint; before, each of the 3 measures and prepaint prepared
    // too (6 preparations, 72 child layouts).
    assert!(
        layouts <= 3 * items as u64,
        "{layouts} child layouts for {items} children"
    );
}
