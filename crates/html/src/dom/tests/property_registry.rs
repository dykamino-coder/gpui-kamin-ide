//! Снимки @property сохраняют изоляцию документов и правила наследования.

use super::*;
use crate::style::values::value::Len;

#[test]
fn document_snapshots_keep_registration_and_inheritance_isolated() {
    let markup = "<!doctype html><section><p></p></section>";
    let registration =
        "@property --size { syntax: '<length>'; inherits: false; initial-value: 7px }";
    let rules = "section { --size: 20px } p { width: var(--size, 3px) }";
    let a = parse(markup, &format!("{registration} {rules}"));
    let b = parse(markup, rules);
    let c = parse("<!doctype html><p></p>", "p { width: var(--size, 3px) }");
    fn width(nodes: &[Node]) -> Option<Len> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e) if e.tag == "p" => e.style.cascaded().width,
            Node::Element(e) => width(&e.children),
            _ => None,
        })
    }
    assert_eq!(width(&a), Some(Len::Px(7.0)));
    assert_eq!(width(&b), Some(Len::Px(20.0)));
    assert_eq!(width(&c), Some(Len::Px(3.0)));
    let before = a.doc.property_rules();
    assert_eq!(before.revision, 1);
    assert!(before.rules.contains_key("--size"));
    assert!(b.doc.property_rules().rules.is_empty());
    assert!(c.doc.property_rules().rules.is_empty());
    let inherited = registration.replace("false", "true");
    let d = parse(markup, &format!("{inherited} {rules}"));
    assert_eq!(width(&d), Some(Len::Px(20.0)));
    assert_eq!(width(&a), Some(Len::Px(7.0)));
    assert!(std::rc::Rc::ptr_eq(&before, &a.doc.property_rules()));
}
