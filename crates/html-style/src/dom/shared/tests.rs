//! Tests of the element name and attribute list storage.

use super::{ElementAttrs, Tag};

#[test]
fn a_write_reaches_only_its_own_copy() {
    let a: ElementAttrs = vec![("class".into(), "row".into())].into();
    let mut b = a.clone();
    assert_eq!(a, b);
    b.edit().push(("id".into(), "x".into()));
    assert_eq!(a.len(), 1, "a write does not reach the other copy");
    assert_eq!(b.len(), 2);
    let mut c = a.clone();
    c.edit()[0].1.push_str(" odd");
    assert_eq!((a[0].1.as_str(), c[0].1.as_str()), ("row", "row odd"));
    c.edit().retain(|(name, _)| name != "class");
    assert!(c.is_empty());
    assert_eq!(
        c,
        ElementAttrs::default(),
        "an emptied list is the empty list"
    );
    assert_eq!(a[0].1, "row");
}

#[test]
fn names_and_empty_attribute_lists_cost_no_allocation_per_element() {
    let a: Tag = "div".into();
    let b: Tag = String::from("div").into();
    assert_eq!(a, b);
    assert_eq!(std::mem::size_of::<Tag>(), 8, "a name is one atom");
    let custom: Tag = "x-a-rather-long-custom-element".into();
    assert_eq!(custom.as_str(), "x-a-rather-long-custom-element");
    assert_ne!(custom, a);
    let empty: ElementAttrs = Vec::new().into();
    assert!(empty.is_empty());
    assert_eq!(empty, ElementAttrs::default());
    assert_eq!(
        std::mem::size_of::<ElementAttrs>(),
        16,
        "a list is one pointer and a count"
    );
}

#[test]
fn a_tag_compares_and_hashes_like_its_name() {
    let tag: Tag = "div".into();
    assert!(tag == "div");
    assert!("div" == tag);
    assert_eq!(tag.as_str(), "div");
    assert_eq!(String::from(tag.clone()), "div");
    let names: std::collections::HashSet<Tag> = [tag].into_iter().collect();
    assert!(names.contains("div"));
    let (a, b): (Tag, Tag) = ("a".into(), "b".into());
    assert!(a < b, "names order like strings");
    assert_eq!(Tag::default().as_str(), "");
}
