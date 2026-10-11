//! Fold GPUI legacy flags into native Taffy alignment and balanced wrapping.
//! Existing HTML writing-mode projection remains physical and defaults to Ltr.
use taffy::{AlignContent, AlignItems, AlignmentSafety, FlexWrap};

pub(super) fn items(value: Option<AlignItems>, safe: bool) -> Option<AlignItems> {
    value.map(|mut alignment| {
        if safe {
            alignment.safety = AlignmentSafety::Safe;
        }
        alignment
    })
}

pub(super) fn content(value: Option<AlignContent>, safe: bool) -> Option<AlignContent> {
    value.map(|mut alignment| {
        if safe {
            alignment.safety = AlignmentSafety::Safe;
        }
        alignment
    })
}

pub(super) fn flex_wrap(value: FlexWrap, minimum_lines: u16) -> FlexWrap {
    if minimum_lines == 0 {
        return value;
    }
    match value {
        FlexWrap::Wrap => FlexWrap::Balance,
        FlexWrap::WrapReverse => FlexWrap::BalanceReverse,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_alignment_preserves_normal_fallback() {
        assert_eq!(items(None, true), None);
        assert_eq!(content(None, true), None);
    }
    #[test]
    fn safety_preserves_the_first_and_last_baseline_keywords() {
        assert_eq!(
            items(Some(AlignItems::BASELINE), true).unwrap().keyword,
            AlignItems::BASELINE.keyword
        );
        assert_eq!(
            items(Some(AlignItems::LAST_BASELINE), true)
                .unwrap()
                .keyword,
            AlignItems::LAST_BASELINE.keyword
        );
        assert_eq!(
            items(Some(AlignItems::END), true).unwrap().safety,
            AlignmentSafety::Safe
        );
        assert_eq!(
            content(Some(AlignContent::CENTER), false),
            Some(AlignContent::CENTER)
        );
    }
    #[test]
    fn balancing_preserves_reverse_and_nowrap_semantics() {
        assert_eq!(flex_wrap(FlexWrap::Wrap, 3), FlexWrap::Balance);
        assert_eq!(
            flex_wrap(FlexWrap::WrapReverse, 3),
            FlexWrap::BalanceReverse
        );
        assert_eq!(flex_wrap(FlexWrap::NoWrap, 3), FlexWrap::NoWrap);
        assert_eq!(flex_wrap(FlexWrap::WrapReverse, 0), FlexWrap::WrapReverse);
    }
}
