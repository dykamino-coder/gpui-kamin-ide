//! Продолжение передаёт детям остаток content box без исходных пределов высоты.
use super::*;

#[test]
fn descendants_use_the_content_remainder_as_the_percentage_basis() {
    let tree = crate::dom::parse(
        "<section style='height:80px;min-height:150px;max-height:180px;box-sizing:border-box;padding:10px'><article style='height:50%'></article></section>",
        "",
    );
    let opts = opts(&tree);
    let mut section = target(&tree).clone();
    section.marks.spanner_content_box = true;
    let resolved = resolve(
        &section,
        &opts.root_style(),
        Some(400.0),
        Some(200.0),
        &opts,
    );
    let Node::Element(child) = &resolved.children[0] else {
        panic!("article child")
    };
    assert_eq!(child.style.used().height, Some(Len::Px(40.0)));
    assert_eq!(resolved.style.used().min_height, Some(Len::Px(150.0)));
    assert_eq!(resolved.style.used().max_height, Some(Len::Px(180.0)));
    assert_eq!(resolved.style.used().border_box, Some(true));
}
