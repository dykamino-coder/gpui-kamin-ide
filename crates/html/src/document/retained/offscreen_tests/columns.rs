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
    styled(at, edit, "")
}

/// `sources` with item `at` styled `item`.
fn styled(at: usize, edit: &str, item: &str) -> String {
    let items: String = (0..120)
        .map(|i| {
            let (mark, item) = if i == at { (edit, item) } else { ("", "") };
            format!(
                "<li style='{item}'><span class=t><cite><a class=e href=#y>Title of the work {i}</a>. \
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
fn a_band_on_a_box_no_fragment_places_waits_in_its_ancestor_ink_and_shows_as_fresh(
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

/// The root scrolled by `dy` pixels in one step.
fn scrolled(cx: &mut VisualTestContext, dy: f32) {
    use gpui::{ScrollDelta, ScrollWheelEvent, point};
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-dy))),
        ..Default::default()
    });
    frame(cx);
}

/// The root scrolled to the first item pulled up by `margin-top:-1000px`.
fn to_pulled(_: &gpui::Entity<View>, cx: &mut VisualTestContext) {
    scrolled(cx, 950.);
}

/// The edited frame and the one after `then` equal a fresh build's.
fn same_as_fresh(
    cx: &mut TestAppContext,
    page: impl Fn(&str) -> String + Copy,
    then: fn(&gpui::Entity<View>, &mut VisualTestContext),
    waits: bool,
) {
    let (kept, waiting, edited, fresh) = edited(cx, page, then);
    assert!(!fresh.1.is_empty(), "empty reference scene");
    match waits {
        true => assert!(kept && waiting > 0, "kept {kept}, edits waiting {waiting}"),
        false => assert_eq!(waiting, 0, "an edit on the screen or of no area waits"),
    }
    assert_eq!(
        edited.0, fresh.0,
        "the edited frame differs from a fresh one"
    );
    assert_eq!(edited.1, fresh.1, "shown, it differs from a fresh one");
}

#[gpui::test]
fn a_band_on_an_item_pulled_above_its_list_waits_in_the_list_ink_and_shows_as_fresh(
    cx: &mut TestAppContext,
) {
    // Pulled 1000px above the list, still below the screen: the list's
    // ink holds it, so the edit waits, and the scroll to it builds.
    let pulled = |e: &str| styled(0, e, "margin-top:-1000px");
    same_as_fresh(cx, pulled, to_pulled, true);
    same_as_fresh(cx, pulled, jumped, true);
    // Pulled onto the screen: the edit shows at once.
    let shown = |e: &str| styled(0, e, "margin-top:-1950px");
    same_as_fresh(cx, shown, jumped, false);
}

#[gpui::test]
fn a_band_on_a_box_of_vertical_lines_far_below_shows_as_fresh(cx: &mut TestAppContext) {
    // Turned lines paint outside the root environment: their ink is unknown.
    let page = |e: &str| styled(100, e, "writing-mode:vertical-rl;height:12em");
    same_as_fresh(cx, page, jumped, false);
}
