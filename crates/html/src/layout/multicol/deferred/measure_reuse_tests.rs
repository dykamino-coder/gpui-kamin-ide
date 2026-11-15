//! A stack whose width the layout decides (a flex item) lays its columns out
//! once per frame: prepaint takes the columns of the final measure.
//! Counted with `crate::work`; run with `--features work-counters`.

use crate::work::{Counter, Session, Snapshot};

fn total(snapshot: &Snapshot, counter: Counter) -> u64 {
    snapshot.0.iter().map(|phase| phase[counter as usize]).sum()
}

fn page(items: usize) -> String {
    let item = "<p style='margin:0 0 3px'>One two three four five six seven eight.</p>";
    format!(
        "<style>body{{margin:0;font:16px Arial}}</style>\
         <div style='float:left'><div style='columns:3 150px;column-gap:10px'>{}</div></div>",
        item.repeat(items)
    )
}

/// Child layouts and kept measures of one frame, and the frame's scene.
fn frame(cx: &mut gpui::TestAppContext, html: &str) -> (u64, u64) {
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
        total(&work, Counter::MulticolChildLayouts),
        total(&work, Counter::MulticolMeasureReuses),
    )
}

#[gpui::test]
fn a_flex_item_stack_lays_its_columns_out_once_per_frame(cx: &mut gpui::TestAppContext) {
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
