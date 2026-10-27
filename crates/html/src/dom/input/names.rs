//! Набор допущенных имён ревизии: до интернирования, один раз на написание.

use crate::budget::{BudgetKind, LiveReservation, RevisionBudget};
use crate::dom::outcome::FailureReason;
use std::mem;

struct Name {
    spelling: String,
    _storage: LiveReservation,
}

#[derive(Default)]
pub(super) struct Names {
    entries: Vec<Name>,
    storage: Option<LiveReservation>,
}

impl Names {
    pub(super) fn admit(
        &mut self,
        name: &str,
        budget: &RevisionBudget,
    ) -> Result<(), FailureReason> {
        // Упорядоченный Vec даёт fallible capacity без скрытых узлов BTreeMap.
        // Списываем верхнюю границу сравниваемых байтов до каждого сравнения.
        let (mut low, mut high) = (0, self.entries.len());
        while low < high {
            let mid = low + (high - low) / 2;
            let old = &self.entries[mid].spelling;
            budget
                .consume(
                    BudgetKind::TraversalVisits,
                    old.len().min(name.len()) as u64 + 1,
                )
                .map_err(FailureReason::Budget)?;
            match old.as_str().cmp(name) {
                std::cmp::Ordering::Equal => return Ok(()),
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
            }
        }
        budget
            .consume(BudgetKind::UniqueNames, 1)
            .map_err(FailureReason::Budget)?;
        budget
            .consume(BudgetKind::NameBytes, name.len() as u64)
            .map_err(FailureReason::Budget)?;
        budget
            .consume(BudgetKind::TraversalVisits, name.len() as u64)
            .map_err(FailureReason::Budget)?;
        let storage = budget
            .document()
            .reserve(BudgetKind::LiveBytes, name.len() as u64)
            .map_err(FailureReason::Budget)?;
        let mut spelling = String::new();
        spelling
            .try_reserve_exact(name.len())
            .map_err(|_| FailureReason::Allocation)?;
        spelling.push_str(name);
        self.reserve(budget)?;
        budget
            .consume(
                BudgetKind::TraversalVisits,
                (self.entries.len() - low) as u64,
            )
            .map_err(FailureReason::Budget)?;
        self.entries.insert(
            low,
            Name {
                spelling,
                _storage: storage,
            },
        );
        Ok(())
    }

    fn reserve(&mut self, budget: &RevisionBudget) -> Result<(), FailureReason> {
        if self.entries.len() < self.entries.capacity() {
            return Ok(());
        }
        let capacity = self
            .entries
            .len()
            .checked_add(1)
            .and_then(|n| n.max(4).checked_next_power_of_two())
            .ok_or(FailureReason::CapacityOverflow)?;
        let bytes = capacity
            .checked_mul(mem::size_of::<Name>())
            .ok_or(FailureReason::CapacityOverflow)?;
        let storage = budget
            .document()
            .reserve(BudgetKind::LiveBytes, bytes as u64)
            .map_err(FailureReason::Budget)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity)
            .map_err(|_| FailureReason::Allocation)?;
        budget
            .consume(BudgetKind::TraversalVisits, self.entries.len() as u64)
            .map_err(FailureReason::Budget)?;
        entries.append(&mut self.entries);
        drop(mem::replace(&mut self.entries, entries));
        self.storage = Some(storage);
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::budget::{BudgetError, DocumentBudget, DocumentLimits, ResourceGovernor};

    fn revision(kind: BudgetKind, cap: u64) -> RevisionBudget {
        DocumentBudget::new(
            DocumentLimits::default().lower(kind, cap).unwrap(),
            ResourceGovernor::default(),
        )
        .unwrap()
        .revision()
    }

    #[test]
    fn unique_name_count_has_all_three_boundaries() {
        for count in [7, 8, 9] {
            let budget = revision(BudgetKind::UniqueNames, 8);
            let mut names = Names::default();
            let result = (0..count).try_for_each(|n| names.admit(&format!("n{n}"), &budget));
            assert_eq!(result.is_ok(), count <= 8);
            if let Err(FailureReason::Budget(BudgetError::Exceeded(error))) = result {
                assert_eq!(
                    (error.kind, error.limit, error.consumed, error.increment),
                    (BudgetKind::UniqueNames, 8, 8, 1)
                );
            } else {
                assert!(result.is_ok());
            }
            assert_eq!(budget.used(BudgetKind::UniqueNames), count.min(8));
            drop(names);
            assert_eq!(budget.document().used(BudgetKind::LiveBytes), 0);
            assert_eq!(budget.document().governor().used(BudgetKind::LiveBytes), 0);
        }
    }

    #[test]
    fn name_bytes_have_all_three_boundaries() {
        for bytes in [7, 8, 9] {
            let budget = revision(BudgetKind::NameBytes, 8);
            let mut names = Names::default();
            let result = names.admit(&"a".repeat(bytes), &budget);
            assert_eq!(result.is_ok(), bytes <= 8);
            if let Err(FailureReason::Budget(BudgetError::Exceeded(error))) = result {
                assert_eq!(
                    (error.kind, error.limit, error.increment),
                    (BudgetKind::NameBytes, 8, bytes as u64)
                );
            } else {
                assert!(result.is_ok());
            }
            drop(names);
            assert_eq!(budget.document().used(BudgetKind::LiveBytes), 0);
        }
    }

    #[test]
    fn repeated_spellings_share_admission_across_roles_and_use_utf8_bytes() {
        let budget = revision(BudgetKind::NameBytes, 8);
        let mut names = Names::default();
        for spelling in ["", "urn:r", "я", "p"] {
            for _ in 0..4 {
                names.admit(spelling, &budget).unwrap();
            }
        }
        let live = budget.document().used(BudgetKind::LiveBytes);
        let work = budget.used(BudgetKind::TraversalVisits);
        for spelling in ["p", "я", "urn:r", ""] {
            names.admit(spelling, &budget).unwrap();
        }
        assert_eq!(budget.used(BudgetKind::UniqueNames), 4);
        assert_eq!(budget.used(BudgetKind::NameBytes), 8);
        assert_eq!(budget.document().used(BudgetKind::LiveBytes), live);
        assert!(budget.used(BudgetKind::TraversalVisits) > work);
        drop(names);
        assert_eq!(budget.document().used(BudgetKind::LiveBytes), 0);
        assert_eq!(budget.used(BudgetKind::NameBytes), 8);
    }

    #[test]
    fn every_storage_refusal_releases_partial_names() {
        for cap in 0..1024 {
            let budget = revision(BudgetKind::LiveBytes, cap);
            let mut names = Names::default();
            let _ = ["KEEP", "attr", "urn:namespace", "prefix", "LOST"]
                .into_iter()
                .try_for_each(|name| names.admit(name, &budget));
            drop(names);
            assert_eq!(
                budget.document().used(BudgetKind::LiveBytes),
                0,
                "cap={cap}"
            );
            assert_eq!(budget.document().governor().used(BudgetKind::LiveBytes), 0);
        }
    }

    #[test]
    fn independent_documents_do_not_share_unique_name_counters() {
        let governor = ResourceGovernor::default();
        let limits = DocumentLimits::default()
            .lower(BudgetKind::UniqueNames, 1)
            .unwrap();
        let first = DocumentBudget::new(limits.clone(), governor.clone())
            .unwrap()
            .revision();
        let second = DocumentBudget::new(limits, governor.clone())
            .unwrap()
            .revision();
        let (mut a, mut b) = (Names::default(), Names::default());
        a.admit("KEEP", &first).unwrap();
        b.admit("KEEP", &second).unwrap();
        assert!(a.admit("LOST", &first).is_err());
        b.admit("KEEP", &second).unwrap();
        let retained = second.document().used(BudgetKind::LiveBytes);
        drop(a);
        assert_eq!(governor.used(BudgetKind::LiveBytes), retained);
        assert_eq!(first.document().used(BudgetKind::LiveBytes), 0);
        drop(b);
        assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
    }
}
