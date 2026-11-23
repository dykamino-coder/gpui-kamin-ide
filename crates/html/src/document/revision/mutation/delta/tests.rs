//! Deltas retain net changes for hidden nodes, reorders and old/new parents.

use super::super::{Batch, Mutation, parent};
use crate::document::{Document, delta::SourceChangeKind};
use html5ever::{QualName, ns};
use markup5ever_rcdom::{Handle, NodeData};

fn named(doc: &Document, id: &str) -> Handle {
    let mut pending = vec![doc.source_dom()];
    while let Some(node) = pending.pop() {
        if matches!(&node.data, NodeData::Element {attrs, ..}
            if attrs.borrow().iter().any(|a| &*a.name.local == "id" && &*a.value == id))
        {
            return node;
        }
        pending.extend(node.children.borrow().iter().cloned());
    }
    panic!("missing {id}")
}

fn batch(doc: &Document, records: &[Mutation]) -> Batch {
    Batch::in_document(records, &doc.source_dom(), || {
        doc.state
            .values
            .traversal
            .source()
            .context()
            .continuing(doc.budget().revision())
    })
    .unwrap()
}

fn attr(node: &Handle, local: &str, value: Option<&str>) -> Mutation {
    Mutation::Attribute {
        element: node.clone(),
        name: QualName::new(None, ns!(), local.into()),
        value: value.map(str::to_owned),
    }
}

#[test]
fn a_reverted_scalar_does_not_invalidate_a_reordered_hidden_branch() {
    let doc = Document::builder("<div id=p hidden><p id=a>A</p><p id=b>B</p></div>", "")
        .build()
        .unwrap();
    let (p, a, b) = (named(&doc, "p"), named(&doc, "a"), named(&doc, "b"));
    let mut batch = batch(
        &doc,
        &[
            attr(&a, "class", Some("temporary")),
            attr(&a, "class", None),
            Mutation::InsertBefore {
                parent: doc.source_node(&p).unwrap(),
                child: doc.source_node(&b).unwrap(),
                before: Some(a.clone()),
            },
        ],
    );
    assert!(batch.elements.is_empty());
    assert!(batch.changes_children());
    assert_eq!(batch.delta.changes.len(), 1);
    let change = &batch.delta.changes[0];
    assert_eq!(change.node, p.key());
    assert_eq!(change.kind, SourceChangeKind::Children);
    assert_eq!(change.before_parent, change.after_parent);
    let delta = batch.delta.clone();
    batch.write();
    assert_eq!(p.children.borrow()[0].key(), b.key());
    assert_eq!(
        batch.delta, delta,
        "writing cannot exchange the published journal"
    );
    batch.undo();
    assert_eq!(p.children.borrow()[0].key(), a.key());
    assert_eq!(batch.delta, delta);
}

#[test]
fn scalar_and_child_events_share_the_final_parent_of_a_move() {
    let doc = Document::builder(
        "<main><section id=from><p id=x>A</p></section><aside id=to></aside></main>",
        "",
    )
    .build()
    .unwrap();
    let (from, to, x) = (named(&doc, "from"), named(&doc, "to"), named(&doc, "x"));
    let mut batch = batch(
        &doc,
        &[
            attr(&x, "class", Some("new")),
            Mutation::InsertBefore {
                parent: doc.source_node(&to).unwrap(),
                child: doc.source_node(&x).unwrap(),
                before: None,
            },
        ],
    );
    let changes: Vec<_> = batch
        .delta
        .changes
        .iter()
        .filter(|change| change.node == x.key())
        .collect();
    assert_eq!(changes.len(), 2);
    assert!(
        changes
            .iter()
            .any(|change| matches!(change.kind, SourceChangeKind::Reparent))
    );
    for change in changes {
        assert_eq!(change.before_parent, Some(from.key()));
        assert_eq!(change.after_parent, Some(to.key()));
    }
    assert_eq!(batch.attribute_deltas().count(), 1);
    batch.write();
    assert_eq!(parent(&x).unwrap().key(), to.key());
    batch.undo();
    assert_eq!(parent(&x).unwrap().key(), from.key());
}

#[test]
fn attribute_names_keep_namespace_and_prefix_and_fold_to_the_final_value() {
    let doc = Document::builder("<p id=x data-n=old>A</p>", "")
        .build()
        .unwrap();
    let x = named(&doc, "x");
    let qualified = QualName::new(Some("xml".into()), ns!(xml), "lang".into());
    let batch = batch(
        &doc,
        &[
            attr(&x, "data-n", Some("temporary")),
            attr(&x, "data-n", Some("old")),
            attr(&x, "class", Some("one")),
            attr(&x, "class", Some("two")),
            Mutation::Attribute {
                element: x.clone(),
                name: qualified.clone(),
                value: Some("ru".into()),
            },
        ],
    );
    let entries: Vec<_> = batch.attribute_deltas().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].1,
        &[QualName::new(None, ns!(), "class".into()), qualified]
    );
    assert_eq!(batch.delta.changes[0].node, x.key());
    assert!(!batch.changes_children());
}

#[test]
fn text_records_use_the_source_key_and_drop_a_reverted_write() {
    let doc = Document::builder("<p id=x hidden>old</p>", "")
        .build()
        .unwrap();
    let x = named(&doc, "x");
    let text = x.children.borrow()[0].clone();
    let unchanged = batch(
        &doc,
        &[
            Mutation::CharacterData {
                node: text.clone(),
                data: "temporary".into(),
            },
            Mutation::CharacterData {
                node: text.clone(),
                data: "old".into(),
            },
        ],
    );
    assert!(unchanged.is_empty());
    assert!(unchanged.delta.changes.is_empty());
    let mut changed = batch(
        &doc,
        &[Mutation::CharacterData {
            node: text.clone(),
            data: "new".into(),
        }],
    );
    assert_eq!(changed.delta.changes.len(), 1);
    let change = &changed.delta.changes[0];
    assert_eq!(change.node, text.key());
    assert_eq!(change.kind, SourceChangeKind::CharacterData);
    assert_eq!(
        (change.before_parent, change.after_parent),
        (Some(x.key()), Some(x.key()))
    );
    let delta = changed.delta.clone();
    changed.write();
    changed.undo();
    assert_eq!(changed.delta, delta);
    let NodeData::Text { contents } = &text.data else {
        panic!("text")
    };
    assert_eq!(&**contents.borrow(), "old");
}
