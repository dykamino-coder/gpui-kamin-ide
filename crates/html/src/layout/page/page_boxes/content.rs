//! Узлы содержимого полевых коробок страницы (content: строки и счётчики).

use crate::dom::Node;
use crate::style::computed::Computed;

// CSS Paged Media 3 §populating-margin-boxes uses the ordinary generated
// content model (CSS Content 3 §2), including quotes and inline images.
pub(super) fn content_nodes(
    items: &[crate::style::computed::ContentItem],
    style: &Computed,
    counters: &mut crate::style::generated::counters::Counters,
) -> Vec<Node> {
    use crate::style::computed::ContentItem;
    let mut children = Vec::new();
    let mut run = Vec::new();
    let flush = |run: &mut Vec<ContentItem>,
                 children: &mut Vec<Node>,
                 counters: &mut crate::style::generated::counters::Counters| {
        if !run.is_empty() {
            children.push(Node::Text(crate::dom::content_text(
                run,
                counters,
                &[],
                style.quotes.as_ref(),
                false,
            )));
            run.clear();
        }
    };
    for item in items {
        if let ContentItem::Image(src) = item {
            flush(&mut run, &mut children, counters);
            if let Some(src) = crate::dom::content_image_src(src) {
                let mut image = crate::layout::table::anon::anon_element("img", vec![]);
                image.inline = true;
                image.attrs.push(("src".into(), src));
                children.push(Node::Element(image));
            }
        } else {
            run.push(item.clone());
        }
    }
    flush(&mut run, &mut children, counters);
    children
}
