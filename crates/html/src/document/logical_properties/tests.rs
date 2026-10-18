//! Image orientation survives document preparation for CSS masks.
use crate::document::Document;
use crate::dom::Node;

#[test]
fn mask_orientation_inherits_through_unwrapped_ancestors_and_resets() {
    let doc = Document::new(
        r#"<style>
        html { image-orientation: none }
        .mask { mask-image: url(unused.jpg) }
        .reset { image-orientation: from-image }
        </style><body style="margin:0">
        <div class="mask"></div>
        <section class="reset"><div class="mask"></div></section>
        <section><div class="mask" style="image-orientation:from-image"></div></section>
        </body>"#,
        "",
    );
    fn collect(nodes: &[Node], flags: &mut Vec<Option<bool>>) {
        for node in nodes {
            if let Node::Element(e) = node {
                if e.style.mask_image.is_some() {
                    flags.push(e.style.image_orient_none);
                }
                collect(&e.children, flags);
            }
        }
    }
    let mut flags = Vec::new();
    collect(doc.nodes(), &mut flags);
    assert_eq!(flags, [Some(true), Some(false), Some(false)]);
}

#[test]
fn logical_sides_map_by_the_inherited_mode_without_touching_own_styles() {
    use crate::style::values::value::Len;
    let doc = Document::new(
        r#"<body><div style="writing-mode: sideways-lr; direction: rtl">
        <p id="a" style="margin-inline-start: 15px; padding-block-start: 5px"></p>
        </div></body>"#,
        "",
    );
    fn find(nodes: &[Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.attrs.iter().any(|(k, v)| k == "id" && v == "a") => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    let p = find(doc.nodes()).expect("p");
    // sideways-lr + rtl: inline-start is the top, block-start the left (§6.4).
    assert_eq!(p.style.margin.top, Some(Len::Px(15.0)));
    assert_eq!(p.style.padding.left, Some(Len::Px(5.0)));
    assert_eq!((p.style.writing_mode, p.style.rtl), (None, None));
}
