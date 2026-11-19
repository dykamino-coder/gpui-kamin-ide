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
