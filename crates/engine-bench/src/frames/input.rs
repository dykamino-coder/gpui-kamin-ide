//! Validated viewport, scroll and frame counter inputs.
use super::{Result, WHEEL_STEP};

pub(super) fn viewport(base: (u32, u32), alternate: bool) -> (u32, u32) {
    if alternate {
        (if base.0 == 8192 { 8191 } else { base.0 + 1 }, base.1)
    } else {
        base
    }
}

/// A wheel sample counts only when the step moved the root scroller by the whole step down:
/// an event a nested scroller took, or one at the document's end, scrolls no root.
type Offset = Option<(f32, f32)>;
pub(super) fn scrolled(from: Offset, to: Offset, max: Offset) -> Result<()> {
    let (Some(from), Some(to)) = (from, to) else {
        return Err("scroll mode needs the root scroller offset before and after the step".into());
    };
    if to.0 != from.0 || (to.1 - from.1 - WHEEL_STEP).abs() > 0.5 {
        return Err(format!(
            "the wheel step did not scroll the root by {WHEEL_STEP} px: {from:?} -> {to:?} \
             (root max offset {max:?}: a nested scroller took it, or the document ended)"
        )
        .into());
    }
    Ok(())
}

pub(super) fn delta(before: (u64, u64), after: (u64, u64)) -> Result<(u64, u64)> {
    Ok((
        after
            .0
            .checked_sub(before.0)
            .ok_or("document build counter regressed")?,
        after
            .1
            .checked_sub(before.1)
            .ok_or("document replay counter regressed")?,
    ))
}
