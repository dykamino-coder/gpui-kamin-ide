//! Background preparation must reach deep leaves and resume pending siblings.

use super::*;

#[test]
fn background_urls_at_twenty_thousand_levels_use_a_bounded_native_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let image_node = |url: &str, children: Vec<Node>| {
                let mut element = crate::dom::Element::synthetic(
                    "div",
                    Computed::default(),
                    Default::default(),
                    children,
                );
                element.style.cascaded_mut().reset_background.bg_image =
                    Some(ImageSource::Url(url.into()));
                Node::Element(element)
            };
            let mut nodes = vec![image_node("leaf", vec![])];
            for _ in 0..20_000 {
                nodes = vec![Node::Element(crate::dom::Element::synthetic(
                    "div",
                    Computed::default(),
                    Default::default(),
                    nodes,
                ))];
            }
            nodes.push(image_node("tail", vec![]));
            let nodes = vec![image_node("root", nodes), image_node("after", vec![])];
            let mut urls = HashSet::new();
            collect(&nodes, &mut urls);
            assert_eq!(
                urls,
                ["root", "leaf", "tail", "after"]
                    .map(String::from)
                    .into_iter()
                    .collect()
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
