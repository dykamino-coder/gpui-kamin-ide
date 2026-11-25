//! Content painted through paint records (multicol columns place segments of
//! one recorded flow) replays a root scroll step moved, as content painted
//! in place does: a record leaves what it paints under the nodes open around
//! it unmarked, and its copy is marked under the node where it lands
//! (`gpui::Window::with_copies_marked`).

use super::fixed_tests::{fixed_box, rows};
use super::scroll_replay_tests::{step, wheel_by};
use super::tests::{frame, open};
use gpui::TestAppContext;

fn columns(extra: &str) -> String {
    let rows: String = (0..400)
        .map(|i| format!("<div class=r>row {i}</div>"))
        .collect();
    format!(
        "<style>body{{margin:0}}.c{{columns:2;column-gap:8px}}\
         .r{{height:20px;break-inside:avoid}}.r:nth-child(odd){{background:#eee}}</style>\
         {extra}<div class=c>{rows}</div>"
    )
}

#[gpui::test]
fn wheel_steps_over_columns_replay_without_a_build(cx: &mut TestAppContext) {
    let html = columns(&fixed_box());
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        // The event's own frame replays the step, the next one replays still.
        let (builds, replays) = step(cx, &html, &name, |c| wheel_by(c, dy), &view, vcx);
        assert_eq!((builds, replays), (0, 2), "{name}");
    }
}

/// A viewport-fixed box lifted above the flow by a positive `z-index` is a
/// deferred draw in the space of the node open where it is deferred, which
/// its viewport anchor cancels: a wheel step replays it in place.
#[gpui::test]
fn wheel_steps_over_a_lifted_fixed_box_replay_without_a_build(cx: &mut TestAppContext) {
    let html = rows(&fixed_box().replace("position:fixed;", "position:fixed;z-index:2;"));
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (builds, replays) = step(cx, &html, &name, |c| wheel_by(c, dy), &view, vcx);
        assert_eq!((builds, replays), (0, 2), "{name}");
    }
}
