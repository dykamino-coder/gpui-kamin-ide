//! Допуск CSS/URL всех ранних входов, транзакционный отказ и release partial state.

use super::*;
use crate::budget::{BudgetError, BudgetKind, DocumentLimits, ResourceGovernor};
use std::path::Path;

fn refused(failure: DocumentFailure, kind: BudgetKind) {
    assert!(
        matches!(failure.reason,
        FailureReason::Budget(BudgetError::Exceeded(limit)) if limit.kind == kind),
        "{failure:?}"
    );
}

#[test]
fn css_entries_admit_all_counter_boundaries_before_early_consumers() {
    for (html, theme, xml) in [
        (
            "<style>@media all { p,a {color:red;margin:0} }</style><p>KEEP</p>",
            "",
            false,
        ),
        ("<p style='color:red;margin:0'>KEEP</p>", "", false),
        ("<p>KEEP</p>", "@layer a {p,a {color:red;margin:0}}", false),
        (
            "<svg><style><![CDATA[p,a {fill:red}]]></style><text>KEEP</text></svg>",
            "",
            true,
        ),
        (
            "<div><template shadowrootmode=open><style>p,a {color:red}</style><slot></slot></template><p>KEEP</p></div>",
            "",
            false,
        ),
        (
            "<style>p,a {color:red; broken:'bad\n; margin:0</style><p>KEEP</p>",
            "",
            false,
        ),
    ] {
        let mut builder = Document::builder(html, theme);
        if xml {
            builder.path(Path::new("source.svg"));
        }
        let measured = builder.build().unwrap();
        for kind in [
            BudgetKind::CssBytes,
            BudgetKind::CssTokens,
            BudgetKind::CssRules,
            BudgetKind::CssSelectors,
            BudgetKind::CssComponents,
            BudgetKind::CssDeclarations,
            BudgetKind::CssDepth,
        ] {
            let n = measured.revision_budget().used(kind);
            assert!(n > 0);
            for cap in [n - 1, n, n + 1] {
                if cap > DocumentLimits::default().limit(kind) {
                    continue;
                }
                let governor = ResourceGovernor::default();
                builder
                    .limits(DocumentLimits::default().lower(kind, cap).unwrap())
                    .governor(&governor);
                match builder.build() {
                    Err(error) if cap < n => refused(error, kind),
                    Ok(document) if cap >= n => {
                        assert_eq!(document.revision_budget().used(kind), n);
                        assert!(document.last_failure().is_none());
                    }
                    _ => panic!("wrong CSS boundary {kind:?}: {cap}/{n}"),
                }
                assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
                assert_eq!(governor.used(BudgetKind::Documents), 0);
            }
        }
    }
}

#[test]
fn inert_template_css_is_not_an_active_css_entry() {
    let a = Document::builder(
        "<template><style>p {color:red}</style></template><p>KEEP</p>",
        "",
    )
    .build()
    .unwrap();
    let b = Document::builder("<template></template><p>KEEP</p>", "")
        .build()
        .unwrap();
    assert_eq!(
        a.revision_budget().used(BudgetKind::CssBytes),
        b.revision_budget().used(BudgetKind::CssBytes)
    );
}

#[test]
fn url_sources_fonts_profiles_and_svg_bases_have_all_three_boundaries() {
    for (html, xml) in [
        ("<base href='https://пример.рф/root/'><p>KEEP</p>", false),
        ("<base href='http://['><p>KEEP</p>", false),
        (
            "<style>@font-face {font-family:x;src:url(missing.woff)}</style><p>KEEP</p>",
            false,
        ),
        (
            "<style>@color-profile --x {src:url(missing.icc)}</style><p>KEEP</p>",
            false,
        ),
        (
            "<svg xml:base='nested/'><use href='other.svg#x'/><text>KEEP</text></svg>",
            true,
        ),
    ] {
        let mut builder = Document::builder(html, "");
        if xml {
            builder.path(Path::new("source.svg"));
        } else {
            builder.base_uri("https://example.com/root/");
        }
        let measured = builder.build().unwrap();
        let n = measured.revision_budget().used(BudgetKind::UrlBytes);
        assert!(n > 0);
        for cap in [n - 1, n, n + 1] {
            let governor = ResourceGovernor::default();
            builder
                .limits(
                    DocumentLimits::default()
                        .lower(BudgetKind::UrlBytes, cap)
                        .unwrap(),
                )
                .governor(&governor);
            match builder.build() {
                Err(error) if cap < n => refused(error, BudgetKind::UrlBytes),
                Ok(document) if cap >= n => {
                    assert_eq!(document.revision_budget().used(BudgetKind::UrlBytes), n)
                }
                _ => panic!("wrong URL boundary {cap}/{n}"),
            }
            assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
            assert_eq!(governor.used(BudgetKind::Documents), 0);
        }
    }
}

#[test]
fn css_and_url_refusals_keep_committed_revision_and_release_candidate() {
    for (kind, candidate) in [
        (
            BudgetKind::CssBytes,
            "<style>p { color: red; margin: 0; }</style><p>LOST</p>",
        ),
        (
            BudgetKind::UrlBytes,
            "<base href='https://пример.рф/LOST/'><p>LOST</p>",
        ),
    ] {
        let governor = ResourceGovernor::default();
        let probe = Document::builder("<p>KEEP</p>", "").build().unwrap();
        let cap = probe.revision_budget().used(kind);
        let mut document = Document::builder("<p>KEEP</p>", "")
            .limits(DocumentLimits::default().lower(kind, cap).unwrap())
            .governor(&governor)
            .build()
            .unwrap();
        let (key, nodes, state) = (
            document.salt(),
            document.nodes.as_ptr(),
            document.state.clone(),
        );
        let live = governor.used(BudgetKind::LiveBytes);
        let failure = document.update(candidate, "").unwrap_err();
        refused(failure, kind);
        assert_eq!(failure.report.retained_revision, Some(key));
        assert_eq!(document.salt(), key);
        assert_eq!(document.nodes.as_ptr(), nodes);
        assert!(Rc::ptr_eq(&state, &document.state));
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "KEEP");
        assert_eq!(governor.used(BudgetKind::LiveBytes), live);
        drop((state, document));
        assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
        assert_eq!(governor.used(BudgetKind::Documents), 0);
    }
}

#[test]
fn early_has_and_slot_work_is_fallible_and_releases_partial_state() {
    for html in [
        "<div><p>KEEP</p><p>LOST</p></div>",
        "<div><template shadowrootmode=open><slot></slot></template><p>KEEP</p><p>LOST</p></div>",
    ] {
        let theme = "div:has(p + p) {color:red} ::slotted(p) {color:blue}";
        let probe = Document::builder(html, theme).build().unwrap();
        let n = probe.revision_budget().used(BudgetKind::TraversalVisits);
        for cap in [n - 1, n, n + 1] {
            let governor = ResourceGovernor::default();
            let result = Document::builder(html, theme)
                .limits(
                    DocumentLimits::default()
                        .lower(BudgetKind::TraversalVisits, cap)
                        .unwrap(),
                )
                .governor(&governor)
                .build();
            match result {
                Err(error) if cap < n => refused(error, BudgetKind::TraversalVisits),
                Ok(_) if cap >= n => {}
                _ => panic!("wrong early-pass boundary {cap}/{n}"),
            }
            assert_eq!(governor.used(BudgetKind::LiveBytes), 0);
            assert_eq!(governor.used(BudgetKind::Documents), 0);
        }
    }
}
