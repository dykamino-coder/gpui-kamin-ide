//! URL/IDNA получает только допущенные строки; отказ не оставляет reservations.

use super::*;
use crate::budget::{BudgetError, DocumentBudget, DocumentLimits, ResourceGovernor};

#[test]
fn uri_and_join_admit_three_length_boundaries_before_url_or_idna() {
    let base = Url::parse("https://example.com/").unwrap();
    for value in [
        "https://пример.рф/a",
        "not a valid URL",
        "//пример.рф/a",
        "",
    ] {
        for join in [false, true] {
            let length = value.len() + if join { base.as_str().len() } else { 0 };
            for cap in [length.saturating_sub(1), length, length + 1] {
                let governor = ResourceGovernor::default();
                let budget = DocumentBudget::new(
                    DocumentLimits::default()
                        .lower(BudgetKind::UrlBytes, cap as u64)
                        .unwrap(),
                    governor.clone(),
                )
                .unwrap();
                let revision = budget.revision();
                let admission = Admission::new(revision.clone());
                let result = if join {
                    super::join(&base, value, &admission)
                } else {
                    Source::Uri(value).resolve(&admission)
                };
                if cap < length {
                    assert!(
                        matches!(result, Err(Error::Budget(BudgetError::Exceeded(limit)))
                        if limit.kind == BudgetKind::UrlBytes)
                    );
                    assert_eq!(revision.used(BudgetKind::TraversalVisits), 0);
                } else {
                    let expected = if join {
                        base.join(value).ok()
                    } else {
                        Url::parse(value).ok()
                    };
                    assert_eq!(result.unwrap(), expected);
                }
                drop((admission, revision, budget));
                assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
                assert_eq!(governor.used(BudgetKind::Documents), 0);
            }
        }
    }
}

#[test]
fn path_admission_precedes_absolute_path_allocation_and_url_conversion() {
    let path = Path::new("source.svg");
    let n = path.as_os_str().len();
    let absolute = std::path::absolute(path).unwrap();
    let total = n + absolute.as_os_str().len();
    for boundary in [n, total] {
        for cap in [boundary - 1, boundary, boundary + 1] {
            let budget = DocumentBudget::new(
                DocumentLimits::default()
                    .lower(BudgetKind::UrlBytes, cap as u64)
                    .unwrap(),
                Default::default(),
            )
            .unwrap();
            let revision = budget.revision();
            let result = Source::Path(path).resolve(&Admission::new(revision.clone()));
            if cap < total {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Url::from_file_path(&absolute).ok());
            }
            if cap < n {
                assert_eq!(revision.used(BudgetKind::TraversalVisits), 0);
            }
            assert_eq!(budget.used(BudgetKind::LiveBytes), 0);
        }
    }
}
