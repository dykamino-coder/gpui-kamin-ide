//! Replay of a root-flow block whose positioned boxes paint in the stacking
//! context outside it (`paint::coordinator::capture::jobs`): a relative block
//! with no context of its own and absolute boxes inside. The block's record
//! keeps the paint of each such step-8 job and the replay hands the jobs back
//! to the collector under their keys; the frame equals a fresh document's,
//! paint order included.

use super::layered_replay_tests::{edit, page};
use gpui::TestAppContext;

/// A relative block of no stacking context: a layer of its own (gradient,
/// opacity, transform), a box with a line of text, a border in its colour
/// and a nested absolute box, and a stacking context of z-index: 0.
fn loose(text: &str, colour: &str) -> String {
    format!(
        "<div style='position:relative;height:60px;background:#eee'>\
         <div style='position:absolute;left:4px;top:4px;width:20px;height:20px;\
         background:linear-gradient(30deg,#000080,#400000);transform:rotate(3deg);opacity:0.5'></div>\
         <div style='position:absolute;left:10px;top:10px;width:40px;height:20px;background:{colour};\
         border:2px solid'>{text}\
         <div style='position:absolute;left:6px;top:6px;width:8px;height:8px;background:#0ff'></div></div>\
         <div style='position:absolute;left:14px;top:14px;width:20px;height:20px;background:#ff0;z-index:0'></div>\
         </div>"
    )
}

/// Two such blocks whose boxes overlap: the second block is pulled up over
/// the first, so the order of the jobs of both shows in the frame.
fn pair(first: &str, second: &str) -> String {
    let over = loose(second, "#0a0").replacen(
        "position:relative;",
        "position:relative;top:-40px;left:6px;",
        1,
    );
    format!("{}{over}", loose(first, "#f00"))
}

#[gpui::test]
fn a_relative_block_replays_with_the_jobs_it_gave_to_the_context(cx: &mut TestAppContext) {
    let block = loose("a", "#f00");
    let (count, _, after, fresh) = edit(cx, &block, &page("", &block, "one"));
    assert_eq!(count, 2, "the relative block and the witness replay");
    assert_eq!(after, fresh, "the replayed jobs differ from a fresh build");
}

#[gpui::test]
fn a_recoloured_block_whose_job_paints_text_is_built(cx: &mut TestAppContext) {
    let block = loose("a", "#f00");
    let (count, before, after, fresh) = edit(cx, &block, &page("alt", &block, "two"));
    assert!(count <= 1, "the border of the job reads the colour");
    assert_eq!(after, fresh);
    assert_ne!(before, after, "the new colour reached the job's border");
}

#[gpui::test]
fn jobs_of_a_built_block_and_a_replayed_one_keep_their_order(cx: &mut TestAppContext) {
    // The first block is built (its text changed), the second replays.
    let (count, before, after, fresh) =
        edit(cx, &pair("a", "b"), &page("", &pair("c", "b"), "one"));
    assert_eq!(count, 2, "the second block and the witness replay");
    assert_eq!(after, fresh, "jobs of the two blocks are out of order");
    assert_eq!(before.len(), after.len());
    // The second block is built, the first replays under it.
    let (count, _, after, fresh) = edit(cx, &pair("a", "b"), &page("", &pair("a", "c"), "one"));
    assert_eq!(count, 2, "the first block and the witness replay");
    assert_eq!(after, fresh, "jobs of the two blocks are out of order");
}

#[gpui::test]
fn a_restyled_job_reaches_the_frame(cx: &mut TestAppContext) {
    let recoloured = format!("{}{}", loose("a", "#00f"), loose("b", "#0a0"));
    let both = format!("{}{}", loose("a", "#f00"), loose("b", "#0a0"));
    let (_, before, after, fresh) = edit(cx, &both, &page("", &recoloured, "one"));
    assert_eq!(after, fresh);
    assert_ne!(before, after, "the new background of the job reached the frame");
}

#[gpui::test]
fn a_job_above_the_step8_boxes_keeps_its_block_built(cx: &mut TestAppContext) {
    // A positive z-index is step 9: a deferred draw of the window, which a
    // record of the block does not hold (`Window::retained_replayable`).
    let raised = loose("a", "#f00").replacen("z-index:0", "z-index:3", 1);
    let block = format!("{raised}{}", loose("b", "#0a0"));
    let (count, _, after, fresh) = edit(cx, &block, &page("", &block, "one"));
    assert_eq!(count, 2, "the block of step-8 boxes and the witness replay");
    assert_eq!(after, fresh, "step 9 lost its place over the replayed jobs");
}