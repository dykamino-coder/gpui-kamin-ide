//! Цветовая правка сохраняет геометрию float/BFC и фрагментированных колонок.

use super::reuse_tests::update;
use super::tests::{frame, open};
use crate::work::{Counter, Session, Snapshot};
use gpui::TestAppContext;

fn total(work: &Snapshot, counter: Counter) -> u64 {
    work.0.iter().map(|phase| phase[counter as usize]).sum()
}

fn layout_counter(columns: bool) -> Counter {
    if columns {
        Counter::MulticolChildLayouts
    } else {
        Counter::FloatPlans
    }
}

fn page(class: &str, columns: bool) -> String {
    let content = if columns {
        "<h3>Section</h3><p>One two three four five six seven eight nine ten.</p>\
         <p>Another paragraph fills the same column row.</p>"
            .repeat(8)
    } else {
        "<div class=float>Float</div><p>One two three four five six seven eight nine ten.</p>"
            .repeat(8)
    };
    let context = if columns {
        "columns:3;column-gap:12px"
    } else {
        "display:flow-root"
    };
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;padding:0;color:#000;font:16px Arial}}\
         html.paint body{{color:#123456}}html.size body{{font-size:20px}}\
         .context{{width:360px;{context}}}p{{margin:0}}\
         .float{{float:left;width:60px;height:36px;margin:2px}}\
         h3{{column-span:all;margin:0;font-size:18px}}\
         </style></head><body><div class=context>{content}</div></body></html>"
    )
}

fn recolour(cx: &mut TestAppContext, columns: bool) {
    let (view, cx) = open(cx, &page("", columns));
    frame(cx);
    update(&view, cx, &page("paint", columns));
    update(&view, cx, &page("", columns));
    let session = Session::begin();
    update(&view, cx, &page("paint", columns));
    let work = session.finish();
    let counter = layout_counter(columns);
    assert_eq!(
        total(&work, counter),
        0,
        "a colour-only edit repeated layout"
    );
    let reused = if columns {
        Counter::MulticolNumericReuses
    } else {
        Counter::FloatNumericReuses
    };
    assert!(total(&work, reused) > 0, "the formatter cache was not used");

    let session = Session::begin();
    update(&view, cx, &page("paint size", columns));
    let work = session.finish();
    assert!(
        total(&work, counter) > 0,
        "a font-size edit reused stale geometry"
    );

    update(&view, cx, &page("paint", columns));
    let session = Session::begin();
    update(
        &view,
        cx,
        &page("paint", columns).replace("One two", "One added two"),
    );
    let work = session.finish();
    assert!(
        total(&work, counter) > 0,
        "a content edit reused stale geometry"
    );

    let session = Session::begin();
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport = (500., 300.)));
    frame(cx);
    let work = session.finish();
    assert!(
        total(&work, counter) > 0,
        "a changed viewport reused stale geometry"
    );
}

#[gpui::test]
fn recolouring_floats_keeps_the_band_plan(cx: &mut TestAppContext) {
    recolour(cx, false);
}

#[gpui::test]
fn recolouring_spanner_rows_keeps_column_measurements(cx: &mut TestAppContext) {
    recolour(cx, true);
}

fn scroll_fallback(cx: &mut TestAppContext, columns: bool) {
    let document = |class| {
        page(class, columns)
            .replace(
                "<body>",
                "<body><div style='overflow:auto;width:300px;height:100px'>",
            )
            .replace("</body>", "</div></body>")
    };
    let (view, cx) = open(cx, &document(""));
    frame(cx);
    update(&view, cx, &document("paint"));
    update(&view, cx, &document(""));
    let session = Session::begin();
    update(&view, cx, &document("paint"));
    let work = session.finish();
    assert!(total(&work, layout_counter(columns)) > 0);
    let reused = if columns {
        Counter::MulticolNumericReuses
    } else {
        Counter::FloatNumericReuses
    };
    assert_eq!(
        total(&work, reused),
        0,
        "scroll recorders require fresh layout"
    );
}

#[gpui::test]
fn recolouring_scroll_floats_rebuilds_layout(cx: &mut TestAppContext) {
    scroll_fallback(cx, false);
}

#[gpui::test]
fn recolouring_scroll_columns_rebuilds_layout(cx: &mut TestAppContext) {
    scroll_fallback(cx, true);
}
