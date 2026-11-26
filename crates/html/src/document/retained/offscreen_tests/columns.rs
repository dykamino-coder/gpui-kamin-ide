//! A band on a nested inline box of a list item in a multi-column list far
//! below the screen (a reference list): the item's paragraph is laid out in
//! a column piece the frame never painted.
use super::*;

/// An ordered list of 300 items in columns of 12em; `edit` goes on the inner
/// span of item `at`, nested in a `cite` in a span.
fn references(at: usize, edit: &str) -> String {
    let items: String = (0..300)
        .map(|i| {
            let mark = if i == at { edit } else { "" };
            format!(
                "<li><span class=r><cite>ref {i} <span id=s{i}{mark}>words</span> \
                 more</cite></span> tail {i}</li>"
            )
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         ol{{column-width:12em;font:16px/20px sans-serif;margin:0}}\
         [data-edit]{{background:#eee}}</style></head><body><ol>{items}</ol></body></html>"
    )
}

#[gpui::test]
fn a_band_on_a_nested_box_of_a_column_item_far_below_waits_and_shows_as_fresh(
    cx: &mut TestAppContext,
) {
    check(cx, |e| references(280, e), true);
}

/// A reference list in columns below a spacer: each item names its source
/// with an external link painting a background image (whose paragraph the
/// frame paints without placing fragments); `edit` goes on a nested
/// `nowrap` span of item `at`.
fn sources(at: usize, edit: &str) -> String {
    let items: String = (0..120)
        .map(|i| {
            let mark = if i == at { edit } else { "" };
            format!(
                "<li><span class=t><cite><a class=e href=#y>Title of the work {i}</a>. \
                 <i>Site</i><span class=d>. Retrieved <span class=w id=s{i}{mark}>January 23,\
                 </span> 2020</span>.</cite></span></li>"
            )
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:#fff;color:#000}}\
         .r{{column-width:30em}} ol{{font:16px/20px sans-serif;margin:0 0 0 2em}}\
         a.e{{background:linear-gradient(red,blue) right center/9px no-repeat;padding-right:12px}}\
         .w{{white-space:nowrap}} [data-edit]{{background:#eee}}</style></head><body>\
         <div style='height:2000px'></div><div class=r><ol>{items}</ol></div></body></html>"
    )
}

/// The root scrolled into the list in one step, past its top.
fn jumped(_: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, point};
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-2150.))),
        ..Default::default()
    });
    frame(cx);
}

#[gpui::test]
fn a_band_on_a_box_no_fragment_places_waits_below_its_ancestor_and_shows_as_fresh(
    cx: &mut TestAppContext,
) {
    let page = |e: &str| sources(100, e);
    let (view, vcx) = settled(cx, &page(""), &page(" data-x"));
    window(vcx, 300.);
    update(&view, vcx, &page(""));
    assert!(
        rects(&view, vcx, "s100").is_none(),
        "the frame placed fragments of the edited box"
    );
    let thens: [fn(&gpui::Entity<View>, &mut VisualTestContext); 3] = [grown, tall, jumped];
    for then in thens {
        let (kept, waiting, edited, fresh) = edited(cx, page, then);
        assert!(!fresh.1.is_empty(), "empty reference scene");
        assert!(kept && waiting > 0, "kept {kept}, edits waiting {waiting}");
        assert_eq!(
            edited.0, fresh.0,
            "the edited frame differs from a fresh one"
        );
        assert_eq!(edited.1, fresh.1, "shown, it differs from a fresh one");
    }
}
