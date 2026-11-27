//! Host preference, page support, explicit element support and revision updates.
//! CSS Color Adjustment 1 section 2.1; HTML color-scheme metadata.

use super::tree;
use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::values::value::{Color, SystemColor, system_color};

fn find<'a>(nodes: &'a [Node], id: &str) -> &'a Element {
    fn search<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(el) if el.attrs.iter().any(|(k, v)| k == "id" && v == id) => Some(el),
            Node::Element(el) => search(&el.children, id),
            _ => None,
        })
    }
    search(nodes, id).unwrap()
}

fn page(meta: &str, direction: &str) -> String {
    format!(
        r#"<!doctype html><html dir={direction}>{meta}<style>
      #query {{ color:red }} @media(prefers-color-scheme:dark) {{ #query {{ color:blue }} }}
      .pair {{ color:light-dark(red,blue) }}
      .light {{ color-scheme:light }} .dark {{ color-scheme:dark }}
      .both {{ color-scheme:light dark }} .normal {{ color-scheme:normal }}
      </style><body><p id=query>query</p><p id=page class=pair>normal</p>
      <div class=light><p id=light class=pair>light</p>
      <p id=normal class="pair normal">page support</p></div>
      <p id=dark class="pair dark">dark</p><p id=both class="pair both">both</p>
      <a id=link href=#query>link</a><input id=field><button id=button>button</button>
      </body></html>"#
    )
}

fn build(html: &str, dark: bool, width: f32) -> Document {
    Document::builder(html, crate::style::ua::BROWSER_CSS)
        .preferred_color_scheme(dark)
        .viewport((width, 900.))
        .build()
        .unwrap()
}

#[test]
fn metadata_never_replaces_host_media_preference() {
    for preferred in [false, true] {
        for (meta, used) in [
            ("", false),
            ("<meta name=color-scheme content=normal>", false),
            ("<meta name=color-scheme content=light>", false),
            ("<meta name=color-scheme content=dark>", true),
            ("<meta name=color-scheme content='light dark'>", preferred),
        ] {
            for direction in ["ltr", "rtl"] {
                for width in [375., 800., 1440.] {
                    let doc = build(&page(meta, direction), preferred, width);
                    let vcx = doc.state.values.values();
                    for (id, dark) in [
                        ("query", preferred),
                        ("page", used),
                        ("normal", used),
                        ("light", false),
                        ("dark", true),
                        ("both", preferred),
                    ] {
                        let el = find(doc.nodes(), id);
                        let expected = Color::parse(if dark { "blue" } else { "red" }, vcx);
                        assert_eq!(
                            el.style.cascaded().inh.ui.color,
                            expected,
                            "{id}: {preferred}/{meta}/{direction}/{width}"
                        );
                    }
                    assert_eq!(doc.state.media().dark, preferred);
                    assert_eq!(
                        find(doc.nodes(), "link").style.cascaded().inh.ui.color,
                        Some(system_color(SystemColor::LinkText, used))
                    );
                    for id in ["field", "button"] {
                        assert_eq!(
                            find(doc.nodes(), id)
                                .style
                                .cascaded()
                                .reset_cascade
                                .dark_color_scheme,
                            used
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn appearance_changes_and_later_attribute_edits_match_fresh_builds() {
    for meta in [
        "",
        "<meta name=color-scheme content=dark>",
        "<meta name=color-scheme content='light dark'>",
    ] {
        let html = page(meta, "rtl");
        let mut doc = build(&html, false, 800.);
        for dark in [true, false, true] {
            assert!(doc.set_preferred_color_scheme(dark).unwrap());
            assert_eq!(tree(&doc), tree(&build(&html, dark, 800.)));
            assert!(!doc.set_preferred_color_scheme(dark).unwrap());
            let edited = html.replace("id=page class=pair", "id=page class='pair light'");
            let records = doc.mutations(&edited).unwrap();
            doc.apply(&records, crate::style::ua::BROWSER_CSS).unwrap();
            assert_eq!(tree(&doc), tree(&build(&edited, dark, 800.)));
            let records = doc.mutations(&html).unwrap();
            doc.apply(&records, crate::style::ua::BROWSER_CSS).unwrap();
        }
    }
}

#[test]
fn print_selection_preserves_the_explicit_host_preference() {
    let doc = Document::builder("<p>x</p>", "")
        .preferred_color_scheme(true)
        .print(true)
        .build()
        .unwrap();
    assert!(doc.state.media().dark && doc.state.media().print);
}

#[test]
fn a_rejected_appearance_change_keeps_the_committed_environment() {
    use crate::budget::{BudgetKind, DocumentLimits};
    let html = "<!doctype html><style>@media(prefers-color-scheme:dark){\
        p{--c:green;color:var(--c)}}</style><p>text</p>";
    let mut doc = Document::builder(html, "")
        .preferred_color_scheme(false)
        .limits(
            DocumentLimits::default()
                .lower(BudgetKind::SubstitutionBytes, 1)
                .unwrap(),
        )
        .build()
        .unwrap();
    let before = tree(&doc);
    assert!(doc.set_preferred_color_scheme(true).is_err());
    assert!(!doc.state.media().dark);
    assert_eq!(tree(&doc), before);
    assert!(doc.last_failure().is_some());
    assert!(!doc.set_preferred_color_scheme(false).unwrap());
}
