//! Тесты: обновление документа проходит тот же жизненный цикл ресурсов и кэша, что и первая
//! сборка.

use super::Document;
use crate::text::fonts;

#[test]
fn update_reloads_resources_and_matches_a_fresh_document() {
    let fonts = crate::text::metrics::FontContext::default()
        .with_loader(|bytes| Some(format!("UpdateFont{}", bytes.last().unwrap())));
    let old = "<style>@font-face {font-family: OldUpdate;\
        src: url(data:font/ttf;base64,AAEAADE=)}</style><p>old</p>";
    let new = "<style>@font-face {font-family: NewUpdate;\
        src: url(data:font/ttf;base64,AAEAADI=)}</style><p>new</p>";
    let mut updated = Document::builder(old, "")
        .fonts(&fonts)
        .base_uri("file:///tests/page.html")
        .build()
        .unwrap();
    assert_eq!(
        fonts::alias("OldUpdate", updated.state.values()).as_deref(),
        Some("UpdateFont49")
    );
    let old_salt = updated.salt();
    assert!(updated.update(new, "p {color: red}").unwrap().changed());
    assert_ne!(old_salt, updated.salt());
    assert_eq!(fonts::alias("OldUpdate", updated.state.values()), None);
    assert_eq!(
        fonts::alias("NewUpdate", updated.state.values()).as_deref(),
        Some("UpdateFont50")
    );
    let fresh = Document::builder(new, "p {color: red}")
        .fonts(&fonts)
        .base_uri("file:///tests/page.html")
        .build()
        .unwrap();
    assert_eq!(updated.salt(), fresh.salt());
    assert_eq!(updated.node_count(), fresh.node_count());
    assert_eq!(
        super::test_tree::tree(&updated),
        super::test_tree::tree(&fresh)
    );
    assert_eq!(
        format!("{:?}", updated.root_style()),
        format!("{:?}", fresh.root_style())
    );
    let nodes = updated.nodes().as_ptr();
    assert!(!updated.update(new, "p {color: red}").unwrap().changed());
    assert_eq!(nodes, updated.nodes().as_ptr());
}

#[test]
fn update_preserves_xml_parsing() {
    let mut doc = Document::builder("<svg xmlns='http://www.w3.org/2000/svg'><g/></svg>", "")
        .path(std::path::Path::new("update.svg"))
        .build()
        .unwrap();
    let new = "<svg xmlns='http://www.w3.org/2000/svg'><metadata/><g/></svg>";
    assert!(doc.update(new, "").unwrap().changed());
    let fresh = Document::builder(new, "")
        .path(std::path::Path::new("update.svg"))
        .build()
        .unwrap();
    assert_eq!(super::test_tree::tree(&doc), super::test_tree::tree(&fresh));
}

#[test]
fn update_carries_paragraph_measurements_but_not_tree_facts() {
    let mut doc = Document::builder("<p>one</p><p>two</p>", "")
        .build()
        .unwrap();
    let measures = doc.state.paragraph.paragraphs.clone();
    let vertical = doc.state.paragraph.vertical.clone();
    assert!(doc.update("<p>one</p><p>tw0</p>", "").unwrap().changed());
    // The value-keyed paragraph cache is the same one; rotated-subtree sizes of the
    // previous tree are not, and everything else is a fresh revision.
    assert!(std::rc::Rc::ptr_eq(
        &measures,
        &doc.state.paragraph.paragraphs
    ));
    assert!(!std::rc::Rc::ptr_eq(
        &vertical,
        &doc.state.paragraph.vertical
    ));
    let fresh = Document::builder("<p>one</p><p>tw0</p>", "")
        .build()
        .unwrap();
    assert!(!std::rc::Rc::ptr_eq(
        &measures,
        &fresh.state.paragraph.paragraphs
    ));
    assert_eq!(super::test_tree::tree(&doc), super::test_tree::tree(&fresh));
}

/// The preparation of the document's tree after a frame of it is built.
fn prepared(
    cx: &mut gpui::TestAppContext,
    doc: &Document,
) -> (std::rc::Rc<crate::layout::cx::subtree::Preparation>, bool) {
    use gpui::{IntoElement, ParentElement, point, px, size};
    let options = crate::layout::options::RenderOpts {
        text: gpui::TextStyle::default(),
        viewport: (800., 600.),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    cx.add_empty_window()
        .draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
            gpui::div()
                .children(doc.render(&options))
                .into_any_element()
        });
    doc.state
        .layout
        .preparation
        .last()
        .expect("a document tree")
}

#[gpui::test]
fn insert_and_remove_prepare_the_new_tree_with_a_full_walk(cx: &mut gpui::TestAppContext) {
    let (one, two) = ("<p>one</p>", "<p>one</p><div><p>two</p></div>");
    let mut doc = Document::builder(one, "").build().unwrap();
    let (first, _) = prepared(cx, &doc);
    for html in [two, one] {
        let tree = doc.tree;
        assert!(doc.update(html, "").unwrap().changed());
        assert_ne!(tree, doc.tree, "a new tree number");
        let (next, full) = prepared(cx, &doc);
        assert!(full, "{html}: the new tree is walked in full");
        assert!(!std::rc::Rc::ptr_eq(&first, &next));
    }
}

fn element(node: &markup5ever_rcdom::Handle, local: &str) -> Option<markup5ever_rcdom::Handle> {
    use markup5ever_rcdom::NodeData;
    if matches!(&node.data, NodeData::Element { name, .. } if &*name.local == local) {
        return Some(node.clone());
    }
    let children = node.children.borrow();
    children.iter().find_map(|c| element(c, local))
}

#[gpui::test]
fn a_clean_revision_keeps_its_state_and_a_preparation_equal_to_a_fresh_one(
    cx: &mut gpui::TestAppContext,
) {
    // The only revision that replaces the tree under the same layout state
    // (`Plan::Clean`: `class`/`data-*` no rule reads). A text edit is a
    // `Patched` revision with a new state. Under the old address key a
    // Clean tree could take the previous tree's preparation; that tree has
    // the same shape and styles, so the preparation's values are its own.
    let mut doc = Document::builder("<p>one</p><div><p>two</p></div>", "")
        .build()
        .unwrap();
    prepared(cx, &doc);
    let (state, tree) = (doc.state.clone(), doc.tree);
    let p = element(&doc.source_dom(), "p").unwrap();
    let mark = super::Mutation::Attribute {
        element: p,
        name: html5ever::QualName::new(None, html5ever::ns!(), "data-k".into()),
        value: Some("1".into()),
    };
    doc.apply(&[mark], "").unwrap();
    assert!(std::rc::Rc::ptr_eq(&state, &doc.state), "a Clean revision");
    assert_ne!(tree, doc.tree, "a new tree number");
    let (kept, _) = prepared(cx, &doc);
    let fresh = crate::layout::cx::subtree::Preparation::fresh(doc.nodes());
    assert_eq!(kept.unkeyed.0, fresh.unkeyed.0);
    assert_eq!(kept.unkeyed.1, fresh.unkeyed.1);
    assert_eq!(kept.clearance_possible, fresh.clearance_possible);
}
