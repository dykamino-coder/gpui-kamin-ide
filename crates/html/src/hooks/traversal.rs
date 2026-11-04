//! Продолжения областей фрагментации на границе layout/paint.

use gpui::Window;
use std::{collections::HashSet, rc::Rc};

pub(crate) async fn with_replay_pieces_async<R>(
    pieces: Rc<crate::layout::fragment::replay_pieces::ReplayPieces>,
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    crate::paint::continuation::replayed::with_pieces_async(pieces, window, f).await
}
pub(crate) use crate::paint::continuation::{
    with_copy_area_async, with_copy_hits_async, with_copy_segment_async,
};
pub(crate) use crate::paint::coordinator::with_late_jobs_async;
pub(crate) async fn with_clone_hidden_async<R>(
    ids: Rc<HashSet<u64>>,
    window: &mut Window,
    f: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    crate::paint::decorations::clone_hidden::with_hidden_async(ids, window, f).await
}
