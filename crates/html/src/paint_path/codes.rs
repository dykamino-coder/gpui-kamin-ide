//! The DevTools `why` codes of the paint path's refusal counters: a refusal
//! counted by `paint_path::add` is also journaled at its caller's line.

use std::sync::atomic::AtomicU64;

/// The journal code of a refusal counter (`refused_*`); `None` for others.
pub(super) fn refusal(counter: &AtomicU64) -> Option<&'static str> {
    let at = super::counters()
        .iter()
        .position(|c| std::ptr::eq(*c, counter))?;
    CODES[at]
}

/// `paint_path.` and the name of each counter of `counters` that is a refusal.
const CODES: [Option<&str>; super::PATH] = {
    let mut codes = [None; super::PATH];
    codes[5] = Some("paint_path.refused_not_paint");
    codes[6] = Some("paint_path.refused_not_own");
    codes[7] = Some("paint_path.refused_no_layer");
    codes[8] = Some("paint_path.refused_by_layer");
    codes[9] = Some("paint_path.refused_shape");
    codes[10] = Some("paint_path.refused_rebuilt");
    codes[11] = Some("paint_path.refused_plan_state");
    codes[12] = Some("paint_path.refused_plan_text");
    codes[13] = Some("paint_path.refused_document");
    codes
};

#[cfg(test)]
mod tests {
    #[test]
    fn every_refusal_counter_has_its_code_and_no_other_does() {
        for (at, name) in super::super::NAMES[..super::super::PATH].iter().enumerate() {
            let expected = name
                .starts_with("refused_")
                .then(|| format!("paint_path.{name}"));
            assert_eq!(super::CODES[at].map(String::from), expected, "{name}");
        }
    }
}

/// `add` of one refusal whose journal entry also says why (`detail`).
#[track_caller]
pub(crate) fn refuse_with(counter: &AtomicU64, detail: impl FnOnce() -> String) {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if crate::why::enabled()
        && let Some(code) = refusal(counter)
    {
        crate::why::note(code, detail);
    }
}
