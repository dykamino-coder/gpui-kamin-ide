//! The negative z-index run of a block list that its blocks paint below (`blocks_flow`).

pub(in crate::render::blocks) struct BelowRun {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) zs: Vec<i32>,
}
