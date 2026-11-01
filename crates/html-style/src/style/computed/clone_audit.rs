//! Учёт фактических копий операндов, включая COW и временные значения на стеке.

/// Нулевой по размеру маркер: derive(Clone) учитывает каждую копию владельца.
/// Это счётчик копирования, а не оценка аллокаций вложенных строк и векторов.
#[derive(Debug, PartialEq)]
pub struct CloneAudit<const INHERITED: bool>;

impl<const INHERITED: bool> Default for CloneAudit<INHERITED> {
    fn default() -> Self {
        #[cfg(feature = "work-counters")]
        crate::work::style_storage::header::<INHERITED>(true);
        Self
    }
}

#[cfg(feature = "work-counters")]
impl<const INHERITED: bool> Drop for CloneAudit<INHERITED> {
    fn drop(&mut self) {
        crate::work::style_storage::header::<INHERITED>(false);
    }
}

impl<const INHERITED: bool> Clone for CloneAudit<INHERITED> {
    fn clone(&self) -> Self {
        if INHERITED {
            crate::work::add!(InheritedStyleClones, 1);
            crate::work::add!(InheritedStyleCloneBytes, size_of::<super::InheritedStyle>());
        } else {
            crate::work::add!(ComputedClones, 1);
            crate::work::add!(ComputedCloneBytes, size_of::<super::Computed>());
        }
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_has_no_storage_cost() {
        assert_eq!(size_of::<CloneAudit<false>>(), 0);
        assert_eq!(size_of::<CloneAudit<true>>(), 0);
    }

    #[cfg(feature = "work-counters")]
    #[test]
    fn derives_and_inherited_cow_count_separately() {
        use crate::work::{Counter, Phase, Session};
        let session = Session::begin();
        crate::work::phase!(_phase, Style);
        let original = super::super::Computed::default();
        let mut copy = original.clone();
        copy.inh.font.font_weight = Some(700);
        let snapshot = session.finish();
        let counts = snapshot.0[Phase::Style as usize];
        assert_eq!(counts[Counter::ComputedClones as usize], 1);
        assert_eq!(counts[Counter::InheritedStyleClones as usize], 1);
        assert_eq!(
            counts[Counter::ComputedCloneBytes as usize],
            size_of_val(&original) as u64
        );
        assert_eq!(original.inh.font.font_weight, None);
    }
}
