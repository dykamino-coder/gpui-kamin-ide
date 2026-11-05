//! Адаптер числовых проб без приостановок использует общий алгоритм.
use super::super as plan;
use crate::layout::fragment::types::Kid;
use plan::Plan;

#[allow(clippy::too_many_arguments)]
pub(in crate::layout::fragment::plan) fn fill_pieces_inner(
    target_at: &dyn Fn(usize) -> f32,
    inline_at: &dyn Fn(usize) -> Option<f32>,
    limit: usize,
    paged: bool,
    col: &mut usize,
    y: &mut f32,
    not_top: usize,
    placed: &mut bool,
    shortage: &mut f32,
    out: &mut Plan,
    float_hold: &mut Option<(usize, usize, f32)>,
    kid: usize,
    k: &Kid,
    cur: f32,
    from: f32,
    copy: usize,
    flow: f32,
    rg: crate::layout::fragment::types::RepeatGeom,
    remeasure: &mut plan::ResumeMeasure<'_>,
    tokens: &mut plan::cuts::InlineTokens,
    single: bool,
) -> Result<plan::piece::Cursor, crate::layout::fragment::token::Error> {
    super::super::ready::run(super::fill_pieces_inner_async(
        target_at,
        inline_at,
        limit,
        paged,
        col,
        y,
        not_top,
        placed,
        shortage,
        out,
        float_hold,
        kid,
        k,
        cur,
        from,
        copy,
        flow,
        rg,
        &mut async |request| remeasure(request),
        tokens,
        single,
    ))
}
