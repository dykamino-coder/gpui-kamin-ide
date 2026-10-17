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
