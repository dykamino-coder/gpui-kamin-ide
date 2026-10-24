//! Границы токенов, регистр имён языка документа и запасные значения attr().

use super::*;
use crate::document::state::DocState;
use crate::style::css::{Media, parse_decls_in, property::has_substitution};

const ELEMENT: CascadeCx<'static> = CascadeCx {
    sibling: Some((2, 4)),
    ..CascadeCx::NONE
};

#[test]
fn substitution_only_visits_function_tokens() {
    let doc = DocState::new(Media::default());
    for source in [
        r#""sibling-index() attr(n px)""#,
        r"/* sibling-count() attr(n px) */",
        r"sibling-index\(\) attr\(n px\)",
        r"foo-sibling-index() fooattr(n px)",
        r"#attr(n px) @sibling-index()",
        r"url(sibling-index())",
    ] {
        assert_eq!(
            resolve_attrs("width", source, ELEMENT, doc.values()),
            source
        );
        assert_eq!(resolve_sibling(source.into(), ELEMENT), source);
        assert!(!has_substitution(source), "{source}");
    }
    for source in ["SIBLING-INDEX()", r"\73 ibling-index(/*x*/)"] {
        assert_eq!(resolve_sibling(source.into(), ELEMENT).trim(), "2");
        assert!(has_substitution(source));
    }
}

#[test]
fn typed_attr_preserves_language_names_fallbacks_and_token_boundaries() {
    let doc = DocState::new(Media::default());
    let attributes = [("DATA-X".into(), "12".into())];
    let xml = CascadeCx {
        attrs: &attributes,
        ..ELEMENT
    };
    let html = CascadeCx {
        html_attrs: true,
        ..xml
    };
    let resolve = |value, cx| resolve_attrs("width", value, cx, doc.values());
    assert_eq!(resolve("attr(data-x px, 7px)", xml).trim(), "7px");
    assert_eq!(resolve("ATTR(DATA-X px, 7px)", xml).trim(), "12px");
    assert_eq!(resolve(r"\61 ttr(data-x px)", html).trim(), "12px");
    assert_eq!(resolve("attr(data-x/* , ) */px, 7px)", html).trim(), "12px");
    assert_eq!(resolve("attr(absent px)", html), "unset");
    assert_eq!(
        resolve("attr(absent type(*), attr(DATA-X px))", xml).trim(),
        "12px"
    );
    assert_eq!(
        resolve(r#"attr(absent type(*), "),")"#, xml).trim(),
        r#""),""#
    );
    assert_ne!(resolve("attr(DATA-X number)px", xml), "12px");
    assert_ne!(resolve_sibling("sibling-index()px".into(), xml), "2px");
}

#[test]
fn attr_url_taint_is_not_a_cast_failure_or_a_string_search() {
    let doc = DocState::new(Media::default());
    for url in ["url(test.png)", r"\75 rl(test.png)"] {
        let attributes = [("x".into(), url.into())];
        let cx = CascadeCx {
            attrs: &attributes,
            ..ELEMENT
        };
        assert_eq!(
            resolve_attrs(
                "background-image",
                "attr(x type(*), none)",
                cx,
                doc.values()
            ),
            "unset"
        );
    }
    let quoted = r#"attr(missing type(*), "url(test.png)")"#;
    assert_eq!(
        resolve_attrs("font-family", quoted, ELEMENT, doc.values()).trim(),
        r#""url(test.png)""#
    );
    assert_eq!(
        resolve_attrs(
            "background-image",
            "attr(missing type(*), url(test.png))",
            ELEMENT,
            doc.values()
        ),
        "unset"
    );
    let empty = [("x".into(), String::new())];
    assert_eq!(
        resolve_attrs(
            "width",
            "attr(x type(*), 1px)",
            CascadeCx {
                attrs: &empty,
                ..ELEMENT
            },
            doc.values()
        )
        .trim(),
        ""
    );
}

#[test]
fn declaration_normalization_cannot_create_substitution_tokens() {
    let doc = DocState::new(Media::default());
    for source in [r"sibling-index\(\)", r"attr\(n px\)"] {
        let decls = parse_decls_in(&format!("--test: {source}"), doc.values());
        let value = decls.last("--test").unwrap();
        assert!(!has_substitution(value), "{value}");
        assert_eq!(resolve_sibling(value.into(), ELEMENT), value);
    }
}
