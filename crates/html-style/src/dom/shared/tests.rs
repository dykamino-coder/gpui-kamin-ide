use super::Shared;

#[test]
fn copies_share_until_one_is_written() {
    let a: super::SharedAttrs = vec![("class".into(), "row".into())].into();
    let mut b = a.clone();
    assert!(std::ptr::eq(&*a, &*b), "a copy shares the attributes");
    b.edit().push(("id".into(), "x".into()));
    assert_eq!(a.len(), 1, "a write does not reach the other copy");
    assert_eq!(b.len(), 2);
    let mut c = a.clone();
    c.edit()[0].1.push_str(" odd");
    assert_eq!((a[0].1.as_str(), c[0].1.as_str()), ("row", "row odd"));
    c.edit().retain(|(name, _)| name != "class");
    assert!(c.is_empty());
    assert_eq!(a[0].1, "row");
}

#[test]
fn names_and_empty_attribute_lists_are_one_value_per_thread() {
    let a: Shared<String> = "div".into();
    let b: Shared<String> = String::from("div").into();
    assert!(std::ptr::eq(&*a, &*b), "an element name is interned");
    let c: Shared<String> = "divx".into();
    assert_eq!(
        (b.as_str(), c.as_str()),
        ("div", "divx"),
        "another name is another value"
    );
    let long: Shared<String> = "x".repeat(super::MAX_INTERNED_LEN + 1).into();
    let again: Shared<String> = "x".repeat(super::MAX_INTERNED_LEN + 1).into();
    assert!(!std::ptr::eq(&*long, &*again), "long names are not kept");
    let empty: super::SharedAttrs = Vec::new().into();
    assert!(std::ptr::eq(
        empty.as_ptr(),
        super::SharedAttrs::default().as_ptr()
    ));
}

#[test]
fn a_shared_tag_compares_like_a_string() {
    let tag: Shared<String> = "div".into();
    assert!(tag == "div");
    assert!("div" == tag);
    assert_eq!(tag.as_str(), "div");
    assert_eq!(String::from(tag), "div");
}
