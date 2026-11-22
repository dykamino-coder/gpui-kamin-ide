//! Input geometry and whole wheel-step validation.
use super::Result;

pub(super) fn viewport(base: (u32, u32), alternate: bool) -> (u32, u32) {
    if alternate {
        (if base.0 == 8192 { 8191 } else { base.0 + 1 }, base.1)
    } else {
        base
    }
}

/// A wheel sample counts only when the step moved the root scroller by the whole step:
/// an event a nested scroller took, or one at the document's end, scrolls no root.
/// With `clamp` (`--scroll-back`) a step that the document's end clamps counts too.
type Offset = Option<(f32, f32)>;
pub(super) fn scrolled(
    from: Offset,
    to: Offset,
    step: f32,
    max: Offset,
    clamp: bool,
) -> Result<()> {
    let (Some(from), Some(to)) = (from, to) else {
        return Err("scroll mode needs the root scroller offset before and after the step".into());
    };
    let end = max.map_or(f32::INFINITY, |m| m.1);
    let expected = match clamp {
        true => (from.1 + step).clamp(0.0, end.max(0.0)),
        false => from.1 + step,
    };
    if to.0 != from.0 || (to.1 - expected).abs() > 0.5 {
        return Err(format!(
            "the wheel step did not scroll the root by {step} px: {from:?} -> {to:?} \
             (root max offset {max:?}: a nested scroller took it, or the document ended)"
        )
        .into());
    }
    Ok(())
}

/// A scroll run for memory only (`ENGINE_BENCH_SCROLL_MEMORY_ONLY=1`): it
/// reads the working set of each step and neither keeps the step frames (nor
/// touches their spare buffers, two frames a step) nor builds the fresh
/// documents of their check, so the bench adds nothing per step to the memory
/// it reads (the JSON records the skipped check). Its per-step records (timings,
/// work counters, samples, pixel differences) stop after `MEMORY_KEPT_STEPS`:
/// the stderr curve holds every step.
pub(super) fn memory_only() -> bool {
    std::env::var_os("ENGINE_BENCH_SCROLL_MEMORY_ONLY").is_some_and(|v| v == "1")
}

/// Steps of a memory run whose records the bench keeps (`memory_only`).
const MEMORY_KEPT_STEPS: usize = 100;

/// Whether the bench keeps the records of step `index` (1-based): a memory
/// run holds a bounded number, so its own growth does not read as the
/// document's.
pub(crate) fn keeps_step(scroll: bool, index: usize) -> bool {
    !(scroll && memory_only()) || index <= MEMORY_KEPT_STEPS
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
