//! Графы extends/fallback и типизированный отказ вместо ограничения значения.

use super::*;
use crate::budget::admission::Admission;

#[test]
fn long_extends_and_fallback_chains_preserve_the_selected_style() {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let doc = crate::values::DocumentValues::detached(Default::default());
            let mut rules = doc.counter_styles.borrow_mut();
            for n in 0..4096 {
                rules.insert(
                    format!("e{n}"),
                    Raw {
                        system: Some(System::Extends(format!("e{}", n + 1))),
                        ..Raw::default()
                    },
                );
                rules.insert(
                    format!("f{n}"),
                    Raw {
                        system: Some(System::Fixed(1)),
                        symbols: Some(vec!["x".into()]),
                        fallback: Some(format!("f{}", n + 1)),
                        ..Raw::default()
                    },
                );
            }
            for name in ["e4096", "f4096"] {
                rules.insert(
                    name.into(),
                    Raw {
                        system: Some(System::Cyclic),
                        symbols: Some(vec!["KEEP".into()]),
                        ..Raw::default()
                    },
                );
            }
            drop(rules);
            assert_eq!(
                custom_repr(2, "e0", Context::default(), doc.values()).as_deref(),
                Some("KEEP")
            );
            assert_eq!(
                custom_repr(2, "f0", Context::default(), doc.values()).as_deref(),
                Some("KEEP")
            );
            assert!(doc.revision_budget().failure().is_none());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn every_extends_cycle_member_inherits_decimal_directly() {
    let rules = HashMap::from([
        (
            "a".into(),
            Raw {
                system: Some(System::Extends("b".into())),
                prefix: Some("A".into()),
                ..Raw::default()
            },
        ),
        (
            "b".into(),
            Raw {
                system: Some(System::Extends("a".into())),
                suffix: Some("B".into()),
                ..Raw::default()
            },
        ),
        (
            "c".into(),
            Raw {
                system: Some(System::Extends("a".into())),
                ..Raw::default()
            },
        ),
    ]);
    let resolve = |name| {
        resolve_counter_style(&rules, name, &Admission::default())
            .unwrap()
            .unwrap()
    };
    let a = resolve("a");
    assert_eq!((a.prefix.as_str(), a.suffix.as_str()), ("A", ". "));
    let b = resolve("b");
    assert_eq!((b.prefix.as_str(), b.suffix.as_str()), ("", "B"));
    let c = resolve("c");
    assert_eq!((c.prefix.as_str(), c.suffix.as_str()), ("A", ". "));
}

#[test]
fn symbolic_and_additive_values_are_not_silently_capped() {
    for (system, symbols, additive) in [
        (System::Symbolic, vec!["x".into()], vec![]),
        (System::Additive, vec![], vec![(1, "x".into())]),
    ] {
        let algo = Algo::Own(system, symbols, additive);
        assert_eq!(
            super::initial::initial(&algo, 1501, Context::default(), &Admission::default())
                .unwrap(),
            Some("x".repeat(1501))
        );
    }
    let limits = crate::budget::DocumentLimits::default()
        .lower(crate::budget::BudgetKind::SubstitutionBytes, 9)
        .unwrap();
    let budget = crate::budget::DocumentBudget::new(limits, Default::default()).unwrap();
    let revision = budget.revision();
    assert!(super::initial::repeat("x", 10, &Admission::new(revision.clone())).is_err());
    assert!(revision.failure().is_some());
}
