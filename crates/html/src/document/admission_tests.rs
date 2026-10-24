//! Полный DOM при ограниченном представлении и транзакционная смена revision.

use super::*;
use std::path::Path;

fn source(depth: usize, xml: bool) -> String {
    let wrappers = if xml { depth - 1 } else { depth - 3 };
    format!(
        "{}text{}",
        "<div>".repeat(wrappers),
        "</div>".repeat(wrappers)
    )
}

fn source_text(root: &markup5ever_rcdom::Handle) -> String {
    crate::dom::source_walk::descendants(root, true)
        .filter_map(|node| {
            if let markup5ever_rcdom::NodeData::Text { contents } = &node.data {
                Some(contents.borrow().to_string())
            } else {
                None
            }
        })
        .collect()
}

fn view_text(nodes: &[Node]) -> String {
    let mut text = String::new();
    crate::dom::text_content::gather_text(nodes, &mut text);
    text
}

#[test]
fn build_and_direct_entries_report_all_three_boundaries_without_dom_loss() {
    for xml in [false, true] {
        for depth in [63, 64, 65] {
            let source = source(depth, xml);
            let mut builder = Document::builder(&source, "");
            if xml {
                builder.path(Path::new("depth.svg"));
            }
            let doc = builder.build().unwrap();
            let parsed = if xml {
                crate::dom::try_parse_xml(&source, "")
            } else {
                crate::dom::try_parse(&source, "")
            }
            .unwrap();
            assert_eq!(source_text(&doc.source_dom()), "text");
            assert_eq!(source_text(&parsed.doc.source_dom().unwrap()), "text");
            assert_eq!(doc.traversal_limit(), parsed.doc.traversal_limit());
            if depth == 65 {
                let limit = doc.report().degradation.unwrap();
                assert_eq!((limit.limit, limit.attempted), (64, 65));
                assert_eq!(limit.traversal, TraversalKind::BoxBuild);
                assert_eq!(doc.report().status, RevisionStatus::Degraded);
                assert_eq!(view_text(doc.nodes()), "");
                assert!(
                    doc.status_message()
                        .unwrap()
                        .contains("DOM сохранён полностью")
                );
            } else {
                assert_eq!(doc.report().status, RevisionStatus::Complete);
                assert_eq!(view_text(doc.nodes()), "text");
                assert!(doc.status_message().is_none());
            }
        }
    }
}

#[test]
fn deep_update_commits_whole_dom_and_preserves_following_siblings() {
    let mut doc = Document::builder("<p>old</p>", "").build().unwrap();
    let old = doc.salt();
    let source = format!("{}<p>tail</p>", source(1000, false));
    let report = doc.update(&source, "").unwrap();
    assert!(report.changed());
    assert_eq!(report.status, RevisionStatus::Degraded);
    assert_ne!(doc.salt(), old);
    assert_eq!(source_text(&doc.source_dom()), "texttail");
    assert_eq!(view_text(doc.nodes()), "tail");
    let limit = report.degradation;
    assert_eq!(
        doc.update(&source, "").unwrap().status,
        RevisionStatus::Unchanged
    );
    assert_eq!(doc.report().degradation, limit);
    doc.update("<p>new</p>", "").unwrap();
    assert!(doc.traversal_limit().is_none());
}

#[test]
fn rejected_xml_update_preserves_state_nodes_dom_identity_and_options() {
    let mut doc = Document::builder("<div>old</div>", "div {color:red}")
        .path(Path::new("old.svg"))
        .print(true)
        .build()
        .unwrap();
    let (state, dom, key, nodes) = (
        doc.state.clone(),
        doc.source_dom(),
        doc.salt(),
        doc.nodes.as_ptr(),
    );
    let failure = doc.update("<div>", "div {color:blue}").unwrap_err();
    assert!(matches!(failure.reason, FailureReason::XmlSyntax { .. }));
    assert_eq!(failure.code(), "KAMINIDE_XML_SYNTAX");
    assert_eq!(failure.report.retained_revision, Some(key));
    assert_eq!(doc.report(), failure.report);
    assert!(Rc::ptr_eq(&state, &doc.state));
    assert!(Rc::ptr_eq(&dom, &doc.source_dom()));
    assert_eq!(doc.nodes.as_ptr(), nodes);
    assert_eq!(doc.salt(), key);
    assert!(doc.is_print() && doc.xml);
    assert_eq!(view_text(doc.nodes()), "old");
    assert!(
        doc.status_message()
            .unwrap()
            .contains("отображается предыдущая версия")
    );
    doc.update("<div>new</div>", "").unwrap();
    assert!(doc.last_failure().is_none());
    assert_eq!(view_text(doc.nodes()), "new");
}

#[test]
fn embedded_degradation_is_typed_scoped_and_retains_the_whole_dom() {
    let embedded = parse_embedded_sized(
        &source(65, false),
        "",
        (100., 100.),
        false,
        false,
        true,
        None,
        &Default::default(),
    )
    .unwrap();
    let limit = embedded.state.traversal_limit().unwrap();
    assert_eq!(limit.scope, FailureScope::EmbeddedDocument);
    assert_eq!(limit.code(), "KAMINIDE_LIMIT_TRAVERSAL_DEPTH");
    assert_eq!(source_text(&embedded.state.source_dom().unwrap()), "text");
    assert_eq!(view_text(&embedded.nodes), "");
}

#[test]
fn deep_style_metadata_is_read_without_recursing_or_losing_the_tail() {
    let html = format!(
        "{}<style>p {{color: red}}</style>{}<p>tail</p>",
        "<div>".repeat(1000),
        "</div>".repeat(1000)
    );
    let doc = Document::builder(&html, "div:has(p) {color:blue}")
        .build()
        .unwrap();
    assert!(doc.traversal_limit().is_some());
    assert!(source_text(&doc.source_dom()).ends_with("tail"));
    assert!(view_text(doc.nodes()).ends_with("tail"));
}

#[test]
fn shadow_fallback_walks_cannot_reset_the_stack_guard() {
    let nested = format!(
        "{}deep{}",
        "<div><template shadowrootmode=open><slot>".repeat(70),
        "</slot></template></div>".repeat(70)
    );
    let html = format!("{nested}<p>tail</p>");
    let doc = Document::builder(&html, "").build().unwrap();
    assert!(doc.traversal_limit().is_some());
    assert_eq!(source_text(&doc.source_dom()), "deeptail");
    assert_eq!(view_text(doc.nodes()), "tail");
    let inert = format!("<template>{}</template><p>tail</p>", source(1000, false));
    let doc = Document::builder(&inert, "").build().unwrap();
    assert!(doc.traversal_limit().is_none());
    assert_eq!(source_text(&doc.source_dom()), "texttail");
}

#[test]
fn assigned_slots_and_reversed_counter_prepasses_report_their_limits() {
    let deep = source(1000, false);
    let html = format!(
        "<div><template shadowrootmode=open><slot></slot></template>{deep}</div><p>tail</p>"
    );
    let doc = Document::builder(&html, "").build().unwrap();
    assert!(doc.traversal_limit().is_some());
    assert_eq!(source_text(&doc.source_dom()), "texttail");
    assert_eq!(view_text(doc.nodes()), "tail");
    let html = format!("<div style='counter-reset:reversed(n)'>{deep}</div><p>tail</p>");
    let doc = Document::builder(&html, "").build().unwrap();
    assert_eq!(
        doc.traversal_limit().unwrap().traversal,
        TraversalKind::Counters
    );
    assert_eq!(source_text(&doc.source_dom()), "texttail");
    assert_eq!(view_text(doc.nodes()), "tail");
}
