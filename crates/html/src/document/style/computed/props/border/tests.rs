//! Конкуренция логических и физических сторон после подготовки документа.

use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::values::value::Len;

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.attrs.iter().any(|(k, v)| k == "id" && v == id) => Some(e),
        Node::Element(e) => find(&e.children, id),
        _ => None,
    })
}

#[test]
fn logical_border_sides_compete_with_physical_ones_by_cascade() {
    let doc = Document::builder(
        r#"<style>
        #a { border-block-start: 4px solid; border-top-width: 1px }
        .b { border-top-width: 1px }
        #b { border-block-start-width: 5px }
        #v { border-inline-start: 7px solid red }
        #p { border-width: 2px; --w: bogus }
        #i { border-block-width: inherit; border-inline: var(--w) solid }
        </style><body><div id="a"></div><div id="b" class="b"></div>
        <div style="writing-mode: vertical-rl"><div id="v"></div></div>
        <div id="p"><div id="i"></div></div></body>"#,
        "",
    )
    .build()
    .unwrap();
    let a = find(doc.nodes(), "a").expect("a");
    assert_eq!(a.style.cascaded().border_width.top, Some(Len::Px(1.0)));
    let b = find(doc.nodes(), "b").expect("b");
    assert_eq!(b.style.cascaded().border_width.top, Some(Len::Px(5.0)));
    // `vertical-rl`: inline-start is the top.
    let v = find(doc.nodes(), "v").expect("v");
    assert_eq!(v.style.cascaded().borders().top, Some(Len::Px(7.0)));
    assert_eq!(v.style.cascaded().borders().left, None);
    let i = find(doc.nodes(), "i").expect("i");
    use crate::style::css::LonghandId as L;
    let marked = i.style.cascaded().inherit_longhands;
    assert!(marked.contains(L::BorderTopWidth) && marked.contains(L::BorderBottomWidth));
    // Invalid after substitution: every longhand of the shorthand is unset.
    assert_eq!(i.style.cascaded().border_side_styles[3], Some(0));
}
