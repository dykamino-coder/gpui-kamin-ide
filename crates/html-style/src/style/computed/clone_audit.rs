//! Учёт фактических копий операндов, включая COW и временные значения на стеке.

/// Без instrumentation маркер нулевой; наблюдаемый заголовок удерживает журнал документа.
#[derive(Debug, Default)]
pub struct CloneAudit<const INHERITED: bool> {
    #[cfg(feature = "work-counters")]
    owner: Option<crate::work::style_storage::Owner>,
}
impl<const INHERITED: bool> PartialEq for CloneAudit<INHERITED> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
#[cfg(feature = "work-counters")]
impl<const INHERITED: bool> CloneAudit<INHERITED> {
    pub(super) fn track(&mut self, owner: &crate::work::style_storage::Owner) {
        assert!(self.owner.is_none());
        owner.header::<INHERITED>(true);
        self.owner = Some(owner.clone());
    }
}
#[cfg(feature = "work-counters")]
impl<const INHERITED: bool> Drop for CloneAudit<INHERITED> {
    fn drop(&mut self) {
        if let Some(owner) = &self.owner {
            owner.header::<INHERITED>(false);
        }
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
        #[cfg(feature = "work-counters")]
        {
            if let Some(owner) = &self.owner {
                owner.header::<INHERITED>(true);
            }
            Self {
                owner: self.owner.clone(),
            }
        }
        #[cfg(not(feature = "work-counters"))]
        Self::default()
    }
}

#[cfg(test)]
mod tests {

    #[cfg(not(feature = "work-counters"))]
    #[test]
    fn audit_has_no_storage_cost() {
        assert_eq!(size_of::<super::CloneAudit<false>>(), 0);
        assert_eq!(size_of::<super::CloneAudit<true>>(), 0);
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
