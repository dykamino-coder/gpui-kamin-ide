//! Normal content и slice-края имеют независимые адреса при новой геометрии.

use super::*;

pub(super) fn record(token: &mut Token, height: f32, edges: (f32, f32), offset: f32) {
    let content = (height - edges.0 - edges.1).max(0.0);
    let start = offset.clamp(0.0, edges.0);
    let end = (offset - edges.0 - content).clamp(0.0, edges.1);
    let fraction = |used, total| if total == 0.0 { 1.0 } else { used / total };
    token.consumed = (offset - edges.0).clamp(0.0, content);
    let FcState::Block(state) = &mut token.fc else {
        unreachable!("parallel recorder creates block tokens")
    };
    state.content_block_size = Some(content);
    state.sliced_edges = Some((fraction(start, edges.0), fraction(end, edges.1)));
}

pub(super) fn offset(token: &Token, height: f32, edges: (f32, f32)) -> f32 {
    let FcState::Block(state) = &token.fc else {
        unreachable!("parallel recorder creates block tokens")
    };
    let content = (height - edges.0 - edges.1).max(0.0);
    let (start, end) = state.sliced_edges.expect("recorded slice edges");
    let consumed = match state.content_block_size.expect("recorded content size") {
        0.0 => 0.0,
        old => token.consumed / old * content,
    };
    start * edges.0 + consumed + end * edges.1
}
