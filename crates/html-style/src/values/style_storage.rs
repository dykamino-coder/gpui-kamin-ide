//! Начальные группы и журнал удержания принадлежат документу, без скрытого состояния.
use super::DocumentValues;
use crate::style::computed::Computed;

pub(super) struct InitialStyles {
    seed: Computed,
    #[cfg(feature = "work-counters")]
    owner: crate::work::style_storage::Owner,
}
impl Default for InitialStyles {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut seed = Computed::default();
        #[cfg(feature = "work-counters")]
        let owner = crate::work::style_storage::Owner::default();
        #[cfg(feature = "work-counters")]
        seed.track_initial(&owner);
        Self {
            seed,
            #[cfg(feature = "work-counters")]
            owner,
        }
    }
}
impl DocumentValues {
    /// Новый заголовок с общими неизменяемыми начальными группами этого документа.
    pub fn initial_style(&self) -> Computed {
        self.initial_styles.seed.clone()
    }
    /// Живые заголовки и уникальные группы, происходящие от initial этого документа.
    #[cfg(feature = "work-counters")]
    pub fn style_storage(&self) -> crate::work::style_storage::Snapshot {
        self.initial_styles.owner.snapshot()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documents_own_initial_groups_and_changes_detach_locally() {
        let doc = DocumentValues::detached(Default::default());
        let other = DocumentValues::detached(Default::default());
        let base = doc.initial_style();
        let mut changed = doc.initial_style();
        assert!(base.reset_border.ptr_eq(&changed.reset_border));
        assert!(std::rc::Rc::ptr_eq(&base.inh.0, &changed.inh.0));
        assert!(
            !base
                .reset_border
                .ptr_eq(&other.initial_style().reset_border)
        );
        changed.reset_border.border_box = Some(true);
        changed.inh.font.font_weight = Some(700);
        assert_eq!(base, doc.initial_style());
        assert!(!base.reset_border.ptr_eq(&changed.reset_border));
        assert!(base.reset_grid.ptr_eq(&changed.reset_grid));
    }
    #[cfg(feature = "work-counters")]
    #[test]
    fn live_styles_keep_their_original_ledger_after_document_drop() {
        let doc = DocumentValues::detached(Default::default());
        let owner = doc.initial_styles.owner.clone();
        let mut copy = doc.initial_style();
        let before = doc.style_storage();
        copy.reset_border.border_box = Some(true);
        copy.inh.font.font_weight = Some(700);
        let after = doc.style_storage();
        assert_eq!(after.group_allocations, before.group_allocations + 2);
        assert_eq!(after.inherited_headers, before.inherited_headers + 1);
        drop(doc);
        assert_eq!(owner.snapshot().computed_headers, 1);
        assert_eq!(owner.snapshot().group_allocations, 22);
        drop(copy);
        assert_eq!(owner.snapshot(), Default::default());
    }
}
