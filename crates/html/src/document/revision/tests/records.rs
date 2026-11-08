//! Записи мутаций (`Document::apply`): ревизия после записей равна свежей сборке
//! разметки с тем же отличием и деревом коробок, и состоянием документа;
//! записи, которые не меняют каскад, принимаются без него; остальные собирают
//! тот же изменённый исходный DOM заново; отказ сборки не оставляет следов.

use super::super::{ApplyError, Mutation};
use super::{PAGE, state, tree};
use crate::document::{Document, RevisionStatus};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

/// Первый в прямом порядке узел исходного DOM, для которого `hit`.
fn find(node: &Handle, hit: &dyn Fn(&Handle) -> bool) -> Option<Handle> {
    if hit(node) {
        return Some(node.clone());
    }
    node.children.borrow().iter().find_map(|c| find(c, hit))
}

fn text(doc: &Document, content: &str) -> Handle {
    let hit = |n: &Handle| matches!(&n.data, NodeData::Text { contents } if &**contents.borrow() == content);
    find(&doc.source_dom(), &hit).expect("text node")
}

fn element(doc: &Document, local: &str) -> Handle {
    let hit =
        |n: &Handle| matches!(&n.data, NodeData::Element { name, .. } if &*name.local == local);
    find(&doc.source_dom(), &hit).expect("element")
}

fn data(node: &Handle, data: &str) -> Mutation {
    Mutation::CharacterData {
        node: node.clone(),
        data: data.into(),
    }
}

fn attr(element: &Handle, name: &str, value: Option<&str>) -> Mutation {
    Mutation::Attribute {
        element: element.clone(),
        name: html5ever::QualName::new(None, html5ever::ns!(), name.into()),
        value: value.map(Into::into),
    }
}

/// Применить к документу `old` записи `records`: равна ли ревизия свежей сборке
/// `new`, и принята ли она без каскада.
fn applied(old: &str, records: impl Fn(&Document) -> Vec<Mutation>, new: &str) -> bool {
    let mut doc = Document::builder(old, "").build().unwrap();
    let values = doc.state.values.clone();
    let dom = doc.source_dom();
    let records = records(&doc);
    assert!(doc.apply(&records, "").unwrap().changed(), "{new}");
    assert!(
        Rc::ptr_eq(&dom, &doc.source_dom()),
        "records edit the same DOM"
    );
    let fresh = Document::builder(new, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh), "{old} -> {new}");
    assert_eq!(state(&doc), state(&fresh), "{old} -> {new}");
    Rc::ptr_eq(&values, &doc.state.values)
}

#[test]
fn text_and_unmentioned_attribute_records_keep_the_cascade() {
    let edited = PAGE.replace(">d<", ">e<");
    assert!(applied(PAGE, |d| vec![data(&text(d, "d"), "e")], &edited));
    let both = PAGE
        .replace(">b<", ">bb x<")
        .replace("<p id=x>", "<p id=x data-edit=\"1\">");
    let records = |d: &Document| {
        vec![
            data(&text(d, "b"), "q"),
            attr(&element(d, "p"), "class", Some("zz")),
            attr(&element(d, "p"), "class", None),
            data(&text(d, "b"), "bb x"),
            attr(&find_x(d), "data-edit", Some("1")),
        ]
    };
    assert!(applied(PAGE, records, &both));
    // Запись того же значения — пустой пакет: ревизия не меняется.
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let salt = doc.salt();
    let same = vec![data(&text(&doc, "d"), "d")];
    assert_eq!(
        doc.apply(&same, "").unwrap().status,
        RevisionStatus::Unchanged
    );
    assert_eq!(doc.salt(), salt);
}

fn find_x(doc: &Document) -> Handle {
    let hit = |n: &Handle| match &n.data {
        NodeData::Element { attrs, .. } => attrs.borrow().iter().any(|a| &*a.value == "x"),
        _ => false,
    };
    find(&doc.source_dom(), &hit).expect("p#x")
}

/// Записи правки, построенные по документу.
type Records = fn(&Document) -> Vec<Mutation>;

#[test]
fn records_the_cascade_can_see_rebuild_the_same_dom() {
    let cases: [(Records, String); 4] = [
        // Класс, упомянутый листом документа.
        (
            |d| vec![attr(&text_parent(d, "d"), "class", Some("on"))],
            PAGE.replace("<p>d", "<p class=\"on\">d"),
        ),
        // Атрибут, который каскад читает сам.
        (
            |d| vec![attr(&find_x(d), "id", Some("y"))],
            PAGE.replace("<p id=x>", "<p id=y>"),
        ),
        // Пустой по CSS текст и текст листа `<style>`.
        (
            |d| vec![data(&text(d, "d"), " ")],
            PAGE.replace(">d<", "> <"),
        ),
        (
            |d| {
                vec![data(
                    &text(d, "p{color:#123}td{padding:1px}.on{font-weight:bold}"),
                    "p{color:#124}",
                )]
            },
            PAGE.replace(
                "p{color:#123}td{padding:1px}.on{font-weight:bold}",
                "p{color:#124}",
            ),
        ),
    ];
    for (records, new) in cases {
        assert!(!applied(PAGE, records, &new), "{new}");
    }
}

fn text_parent(doc: &Document, content: &str) -> Handle {
    let node = text(doc, content);
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent.expect("parent")
}

#[test]
fn foreign_records_are_refused_without_a_trace() {
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    let other = Document::builder(PAGE, "").build().unwrap();
    let salt = doc.salt();
    let state = doc.state.clone();
    for records in [
        vec![data(&text(&other, "d"), "e")],
        vec![data(&element(&doc, "p"), "e")],
        vec![attr(&text(&doc, "d"), "class", Some("a"))],
    ] {
        assert!(matches!(doc.apply(&records, ""), Err(ApplyError::Foreign)));
        assert_eq!(doc.salt(), salt);
        assert!(Rc::ptr_eq(&state, &doc.state));
    }
}

#[test]
fn refused_rebuild_restores_the_dom_and_the_revision() {
    use crate::budget::{BudgetKind, DocumentLimits, ResourceGovernor};
    let limits = DocumentLimits::default()
        .lower(BudgetKind::SubstitutionBytes, 1)
        .unwrap();
    let mut doc = Document::builder("<div>a</div>", "")
        .limits(limits)
        .governor(&ResourceGovernor::default())
        .build()
        .unwrap();
    let (salt, state, dom) = (doc.salt(), doc.state.clone(), doc.source_dom());
    let div = element(&doc, "div");
    let style = "--a: green; color: var(--a)";
    let refused = doc.apply(&[attr(&div, "style", Some(style))], "");
    assert!(
        matches!(refused, Err(ApplyError::Rejected(_))),
        "{refused:?}"
    );
    assert_eq!(doc.salt(), salt);
    assert!(Rc::ptr_eq(&state, &doc.state));
    assert!(Rc::ptr_eq(&dom, &doc.source_dom()));
    let NodeData::Element { attrs, .. } = &div.data else {
        unreachable!()
    };
    assert!(attrs.borrow().is_empty());
    // Следующий пакет применяется к тому же DOM.
    assert!(
        doc.apply(&[data(&text(&doc, "a"), "b")], "")
            .unwrap()
            .changed()
    );
    assert!(doc.last_failure().is_none());
}

#[test]
fn update_of_the_same_shape_is_the_records_of_its_difference() {
    let mut doc = Document::builder(PAGE, "").build().unwrap();
    assert!(doc.mutations("<p>other</p>").is_none());
    let new = PAGE
        .replace(">d<", ">e<")
        .replace("<td>2", "<td class=\"qq\" data-a=\"1\">2");
    let records = doc.mutations(&new).unwrap();
    assert_eq!(records.len(), 3);
    let values = doc.state.values.clone();
    let node = text(&doc, "d");
    doc.apply(&records, "").unwrap();
    assert!(Rc::ptr_eq(&values, &doc.state.values));
    assert!(Rc::ptr_eq(&node, &text(&doc, "e")));
    let fresh = Document::builder(&new, "").build().unwrap();
    assert_eq!(tree(&doc), tree(&fresh));
    // `update` той же разметкой пишет те же записи в тот же DOM.
    let mut updated = Document::builder(PAGE, "").build().unwrap();
    let node = text(&updated, "d");
    assert!(updated.update(&new, "").unwrap().changed());
    assert!(Rc::ptr_eq(&node, &text(&updated, "e")));
    assert_eq!(tree(&updated), tree(&fresh));
}
