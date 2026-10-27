//! Продолжения absolute/fixed в реальном документе: размещение, clip containing block
//! и общий эффект предка с копией нормального потока.

use super::{quads, top};
use gpui::{Hsla, Pixels};

fn is(color: Option<Hsla>, rgb: u32) -> bool {
    color.is_some_and(|c| c.to_rgb() == gpui::rgb(rgb))
}

fn any(found: &[(gpui::Bounds<Pixels>, Hsla)], rgb: u32) -> bool {
    found.iter().any(|(_, c)| c.to_rgb() == gpui::rgb(rgb))
}

const COLUMNS: &str = "<style>body{margin:0}</style>\
<div style='columns:2;gap:0;width:100px;height:100px;column-fill:auto'>\
<div style='position:relative;height:200px;CLIP'>\
<div style='position:absolute;width:100%;height:300px'>\
<div style='height:200px;background:#00ff00'></div>\
<div style='height:200px;background:#0000ff'></div></div></div></div>";

#[test]
fn continuation_past_its_containing_block_reaches_the_overflow_column() {
    let found = quads(&COLUMNS.replace("CLIP", ""), 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00));
    assert!(is(top(&found, 75., 50.), 0x00ff00));
    // The third fragment (source 200..300) lands in the first overflow column.
    assert!(is(top(&found, 125., 50.), 0x0000ff));
}

#[test]
fn containing_block_overflow_clip_cuts_continuations_before_placement() {
    let found = quads(&COLUMNS.replace("CLIP", "overflow-y:clip"), 400., 200.);
    assert!(is(top(&found, 25., 50.), 0x00ff00));
    assert!(is(top(&found, 75., 50.), 0x00ff00));
    assert!(!any(&found, 0x0000ff), "clipped continuation painted");
    assert_eq!(top(&found, 125., 50.), None);
}
