//! Decorated rows crossing the viewport after a deferred edit.
use super::*;

#[gpui::test]
fn a_box_with_decorations_far_below_waits_and_shows_as_fresh(cx: &mut TestAppContext) {
    check(cx, |e| rows(140, e, "0 0 0 3px #444"), true);
}

#[gpui::test]
fn a_recorded_relative_outline_far_below_waits_and_matches_fresh(cx: &mut TestAppContext) {
    check(
        cx,
        |edit| {
            rows(140, edit, "0 0 0 3px #444")
                .replace(".r{height", ".r{outline:1em solid red;height")
        },
        true,
    );
}

#[gpui::test]
fn a_shadow_reaching_the_screen_from_a_box_below_is_not_left_waiting(cx: &mut TestAppContext) {
    check(cx, |e| rows(18, e, "0 -80px 0 #444"), false);
}

/// Grow both the window clip and the document viewport so the edited row
/// contributes paint. An unedited fresh document is the negative control.
#[gpui::test]
fn the_revealed_row_contributes_its_changed_fill(cx: &mut TestAppContext) {
    let reveal = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.1 = 4200.));
        window(cx, 4200.);
    };

    let page = |edit: &str| {
        rows(140, edit, "0 0 0 3px #444").replace(".r{height", ".r{outline:1em solid red;height")
    };
    let (kept, waiting, result, reference) = edited(cx, page, reveal);
    let (view, unchanged) = open(cx, &page(""));
    window(unchanged, 300.);
    reveal(&view, unchanged);
    assert!(kept && waiting > 0);
    assert!(!reference.1.is_empty());
    assert_ne!(
        reference.1,
        quads(unchanged),
        "the changed row must be visible"
    );
    assert_eq!(result.1, reference.1);
}
