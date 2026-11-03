//! A balanced multicol with spanners lays each row's float host out a bounded
//! number of times per frame: one measure build (repeated sizing queries reuse
//! it), one final build and the painted copy. Its baselines come from the main
//! copy the columns paint, not from spare copies built once per column.
//! Counted with `crate::work`; run with `--features work-counters`.

use crate::work::{Counter, Session, Snapshot};

fn total(snapshot: &Snapshot, counter: Counter) -> u64 {
    snapshot.0.iter().map(|phase| phase[counter as usize]).sum()
}

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

/// Float-host plans, measure-pass reuses and float-host probes of one frame.
fn work(cx: &mut gpui::TestAppContext, html: &str) -> (u64, u64, u64) {
    use gpui::{IntoElement, ParentElement, point, px, size};
    let document = crate::Document::builder(html, crate::BROWSER_CSS)
        .build()
        .expect("test document");
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    let session = Session::begin();
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            gpui::div()
                .children(document.render(&options))
                .into_any_element()
        });
    let work = session.finish();
    (
        total(&work, Counter::FloatPlans),
        total(&work, Counter::SpannerMeasureReuses),
        total(&work, Counter::BandProbeCalls),
    )
}

#[gpui::test]
fn spanner_rows_plan_each_float_host_at_most_three_times(cx: &mut gpui::TestAppContext) {
    let sections = 3;
    let (plans, reuses, probes) = work(cx, &sheet(sections, 12));
    assert!(reuses > 0, "repeated measure queries rebuilt the rows");
    // Measure build, final build, painted main copy: 13 per section before.
    assert!(
        plans <= 3 * sections as u64,
        "{plans} float-host plans for {sections} sections ({probes} probes)"
    );
    // The work grows with the content, not with the number of columns.
    let (more, _, _) = work(cx, &sheet(sections, 24));
    assert_eq!(more, plans, "plans depend on the item count");
}
