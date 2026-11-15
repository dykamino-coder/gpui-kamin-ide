//! A balanced multicol with spanners lays each row's float host out a bounded
//! number of times per frame: once in the measure build (repeated sizing
//! queries reuse it, and so does the final build's size query of each row) and
//! once for the painted copy. Its baselines come from the main copy the columns
//! paint, not from spare copies built once per column.
//! Counted with `crate::work`; run with `--features work-counters`.

use crate::layout::multicol::work_frame::{frame_work, total};
use crate::work::Counter;

const ITEM: &str = "<p class=item><span class=float></span>One two three four five six seven eight nine ten eleven twelve.</p>";

fn sheet(sections: usize, items: usize) -> String {
    let body: String = (0..sections)
        .map(|_| format!("<h2>Section</h2>{}", ITEM.repeat(items)))
        .collect();
    format!(
        "<style>body{{margin:0;font:16px Arial}}*{{box-sizing:border-box}}\
         .sheet{{width:780px;columns:4;column-gap:12px;column-fill:balance}}\
         .item{{margin:0 0 3px;break-inside:avoid}}h2{{column-span:all;font-size:18px;margin:2px}}\
         .float{{float:left;width:30px;height:40px;margin:2px}}</style><div class=sheet>{body}</div>"
    )
}

/// Float-host plans, measure-pass reuses, float-host probes and row-extent
/// reuses of one frame.
fn work(cx: &mut gpui::TestAppContext, html: &str) -> (u64, u64, u64, u64) {
    let work = frame_work(cx, html);
    (
        total(&work, Counter::FloatPlans),
        total(&work, Counter::SpannerMeasureReuses),
        total(&work, Counter::BandProbeCalls),
        total(&work, Counter::SpannerRowReuses),
    )
}

#[gpui::test]
fn spanner_rows_plan_each_float_host_at_most_twice(cx: &mut gpui::TestAppContext) {
    let sections = 3;
    let (plans, reuses, probes, rows) = work(cx, &sheet(sections, 12));
    assert!(reuses > 0, "repeated measure queries rebuilt the rows");
    assert!(
        rows >= sections as u64,
        "the final build re-measured its rows"
    );
    // Measure build and painted main copy: 13 per section on 7de6fc0b,
    // 3 while the final build re-measured every row.
    assert!(
        plans <= 2 * sections as u64,
        "{plans} float-host plans for {sections} sections ({probes} probes)"
    );
    // The work grows with the content, not with the number of columns.
    let (more, _, _, _) = work(cx, &sheet(sections, 24));
    assert_eq!(more, plans, "plans depend on the item count");
}

#[gpui::test]
fn prepaint_paints_the_measure_of_the_rows(cx: &mut gpui::TestAppContext) {
    let (sections, items) = (3, 12);
    let work = frame_work(cx, &sheet(sections, items));
    let adoptions = total(&work, Counter::SpannerMeasureAdoptions);
    let layouts = total(&work, Counter::MulticolChildLayouts);
    assert_eq!(adoptions, 1, "the final build rebuilt the rows");
    // One layout of every row child per frame: the measure's, painted.
    assert!(
        layouts <= (sections * items) as u64,
        "{layouts} child layouts for {} children",
        sections * items
    );
}
