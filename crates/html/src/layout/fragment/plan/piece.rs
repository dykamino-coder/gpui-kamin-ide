//! Общий вызов раскладки кусков: полный проход и независимый шаг одного потока.

use super::Plan;
use super::cuts::InlineTokens;
use crate::layout::fragment::token::Error;
use crate::layout::fragment::types::{Kid, RepeatGeom};

pub(super) enum LoopStep {
    Break,
    Continue,
}

pub(super) struct Cursor {
    pub(super) from: f32,
    pub(super) copy: usize,
    pub(super) complete: bool,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn fill_pieces_async(
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
    rg: RepeatGeom,
    remeasure: &mut impl AsyncFnMut(super::MeasureRequest) -> super::Measurement,
) -> Result<(), Error> {
    super::pieces::fill_pieces_inner_async(
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
        remeasure,
        &mut InlineTokens::default(),
        false,
    )
    .await?;
    Ok(())
}

/// Повторяемый подвал занимает остаток фрагмента, когда подвал зарезервирован.
pub(super) fn foot_height(room_all: f32, height: f32, reserved: f32) -> f32 {
    if reserved > 0.0 {
        (room_all - height).max(reserved)
    } else {
        0.0
    }
}
