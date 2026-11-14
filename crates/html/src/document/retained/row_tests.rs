//! Повтор рядов таблицы (`layout::reuse::replay::group`): правка одной ячейки
//! пересобирает таблицу, но ряды с теми же входами не собираются — их ячейки
//! занимают свои узлы раскладки, а prepaint, краска, строки и данные проб
//! кромок повторяются одной записью на ряд. Кадр равен свежей сборке.

use super::reuse_tests::{settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn groups(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_groups())
}

/// Quads and underlines of the shown frame in paint order.
fn scene(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|q| format!("{q:?}"));
        let lines = window
            .painted_underlines()
            .into_iter()
            .map(|u| format!("{u:?}"));
        quads.chain(lines).collect()
    })
}

fn page(text: &str) -> String {
    format!(
        "<body style='margin:0;display:flow-root'>\
         <table style='width:300px;border-collapse:collapse'>\
         <tr><td style='border:1px solid red'>a</td><td style='background:lime'><u>b</u></td></tr>\
         <tr><td style='border:2px solid blue'>{text}</td><td>d</td></tr>\
         <tr><td style='border:1px solid green'>e</td><td style='background:silver'>f</td></tr>\
         </table><p>tail</p></body>"
    )
}

/// An edit of one cell's text keeps the other rows: they replay, with the
/// collapsed borders and cell backgrounds they reported, and the frame is the
/// fresh build's.
#[gpui::test]
fn rows_with_the_same_inputs_replay_when_a_cell_changes(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    // The settled builds record the rows; later edits replay the clean ones.
    update(&view, cx, &page("one"));
    let before = groups(&view, cx);
    update(&view, cx, &page("two"));
    assert_eq!(
        groups(&view, cx) - before,
        2,
        "the rows around the edited one were not replayed"
    );
    let replay = scene(cx);
    let (_, fresh) = open(cx, &page("two"));
    frame(fresh);
    assert_eq!(replay, scene(fresh), "the frame with replayed rows differs");
}