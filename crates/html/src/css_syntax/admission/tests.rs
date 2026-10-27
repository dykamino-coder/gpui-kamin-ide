//! Границы каждого CSS счётчика, терминальный отказ и release кадров pre-scan.

use super::*;
use crate::budget::{BudgetError, DocumentBudget, DocumentLimits, ResourceGovernor};

fn revision(kind: BudgetKind, cap: usize) -> crate::budget::RevisionBudget {
    DocumentBudget::new(
        DocumentLimits::default().lower(kind, cap as u64).unwrap(),
        ResourceGovernor::default(),
    )
    .unwrap()
    .revision()
}

#[test]
fn every_css_counter_has_three_boundaries() {
    let css = "@media screen { a,b { color:red; & + & { margin:0 } } }";
    let counts = admit(css, &Admission::default()).unwrap();
    for kind in [
        CssBytes,
        CssTokens,
        CssRules,
        CssSelectors,
        CssComponents,
        CssDeclarations,
        CssDepth,
    ] {
        let n = counts.0[kind as usize];
        assert!(n > 0);
        for cap in [n - 1, n, n + 1] {
            let revision = revision(kind, cap);
            let result = admit(css, &Admission::new(revision.clone()));
            if cap < n {
                let Err(Error::Budget(BudgetError::Exceeded(limit))) = result else {
                    panic!("expected {kind:?} refusal");
                };
                assert_eq!(limit.kind, kind);
            } else {
                assert_eq!(result.unwrap().0, counts.0);
            }
            assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
        }
    }
}

#[test]
fn refusal_stops_before_tail_and_releases_partial_stack() {
    let prefix = "a { b { c";
    let cap = admit(prefix, &Admission::default()).unwrap().0[CssTokens as usize];
    let mut work = Vec::new();
    for tail in ["X", "X/* LOST */"] {
        let revision = revision(CssTokens, cap);
        let css = format!("{prefix}{tail}");
        assert!(admit(&css, &Admission::new(revision.clone())).is_err());
        work.push(revision.used(BudgetKind::TraversalVisits));
        assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
    }
    assert_eq!(work[0], work[1]);
}

#[test]
fn opaque_tokens_keep_canonical_recovery_boundaries() {
    for (plain, opaque) in [
        ("a {x: ;}", "a {x: '({[;,@:';}"),
        ("a {x: ;}", "a {x: url(({[;,@:);}"),
        ("a {}", "a {/* ({[;,@: */}"),
    ] {
        let a = admit(plain, &Admission::default()).unwrap();
        let b = admit(opaque, &Admission::default()).unwrap();
        for kind in [CssRules, CssSelectors, CssDeclarations, CssDepth] {
            assert_eq!(a.0[kind as usize], b.0[kind as usize], "{opaque}");
        }
    }
    for css in [
        "a {x:'broken\n; b {y:z}",
        "a {x:url(broken\\) ;}",
        "a {/* EOF",
        "a {x:'EOF",
        "a {x:\\00007b ;}",
    ] {
        assert!(admit(css, &Admission::default()).is_ok());
    }
}

#[test]
fn nested_expansion_is_admitted_before_canonical_ast() {
    let css = format!(
        "a {{ {} color:red; {} }}",
        "& & {".repeat(20),
        "}".repeat(20)
    );
    let revision = revision(CssComponents, 1024);
    assert!(matches!(admit(&css, &Admission::new(revision.clone())),
        Err(Error::Budget(BudgetError::Exceeded(limit))) if limit.kind == CssComponents));
    assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
}

#[test]
fn separate_documents_do_not_share_css_consumption() {
    let a = revision(CssBytes, 3);
    let b = revision(CssBytes, 3);
    admit("a{}", &Admission::new(a.clone())).unwrap();
    admit("b{}", &Admission::new(b.clone())).unwrap();
    assert!(admit("x", &Admission::new(a.clone())).is_err());
    assert_eq!((a.used(CssBytes), b.used(CssBytes)), (3, 3));
}

#[test]
fn selector_chain_boundaries_precede_recursive_ast_and_drop() {
    let css = format!("{} {{ color:red }}", "p > ".repeat(31) + "p");
    let n = admit(&css, &Admission::default()).unwrap().0[CssDepth as usize];
    assert_eq!(n, 32);
    for cap in [n - 1, n, n + 1] {
        let revision = revision(CssDepth, cap);
        assert_eq!(
            admit(&css, &Admission::new(revision.clone())).is_ok(),
            cap >= n
        );
        assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
    }
    let css = format!("{} {{ color:red }}", "p > ".repeat(10_000) + "p");
    let revision = revision(CssDepth, 128);
    assert!(admit(&css, &Admission::new(revision.clone())).is_err());
    assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
}

#[test]
fn list_expansion_and_declaration_copies_are_charged_before_ast() {
    let parents = (0..64)
        .map(|n| format!(".parent{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let children = (0..64)
        .map(|n| format!(".child{n}"))
        .collect::<Vec<_>>()
        .join(",");
    let css = format!(
        "{parents} {{ {children} {{ --payload:'{}'; }} }}",
        "x".repeat(1024)
    );
    let counts = admit(&css, &Admission::default()).unwrap();
    assert!(counts.0[CssComponents as usize] >= 64 * (parents.len() + 1024));
    let revision = revision(CssComponents, 4096);
    assert!(matches!(admit(&css, &Admission::new(revision.clone())),
        Err(Error::Budget(BudgetError::Exceeded(limit))) if limit.kind == CssComponents));
    assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
}

#[test]
fn dotted_layer_prefix_storage_is_admitted_before_registry_growth() {
    for end in [";", " { p { color:red } }"] {
        let css = format!("/* recovery */ @LaYeR {}last{end}", "segment.".repeat(256));
        let counts = admit(&css, &Admission::default()).unwrap();
        assert!(counts.0[CssComponents as usize] >= 256 * 256 * 8);
        let revision = revision(CssComponents, 4096);
        assert!(admit(&css, &Admission::new(revision.clone())).is_err());
        assert_eq!(revision.document().used(BudgetKind::LiveBytes), 0);
    }
}
