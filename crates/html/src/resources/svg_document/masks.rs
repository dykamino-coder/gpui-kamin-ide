//! SVG-ресурсы принадлежат документу; отложенные группы хранят неизменяемые определения.

use crate::document::state::ValueCx;
use crate::dom::Node;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct MaskResources {
    definitions: RefCell<Arc<MaskDefinitions>>,
    prepared: Cell<bool>,
}

#[derive(Default)]
pub(crate) struct MaskDefinitions(HashMap<String, MaskDefinition>);

pub(crate) struct MaskDefinition {
    pub(crate) markup: String,
    pub(crate) alpha: bool,
}

impl MaskDefinitions {
    pub(crate) fn get(&self, id: &str) -> Option<&MaskDefinition> {
        self.0.get(id)
    }
}

impl MaskResources {
    pub(crate) fn definitions(&self) -> Arc<MaskDefinitions> {
        self.definitions.borrow().clone()
    }

    pub(crate) fn markup(&self, id: &str) -> Option<String> {
        self.definitions.borrow().get(id).map(|d| d.markup.clone())
    }

    pub(crate) fn prepare(&self, nodes: &[Node], vcx: ValueCx<'_>) {
        if !self.prepared.get() {
            self.collect(nodes, vcx);
        }
    }

    /// Rebuild before tree construction, including updates at the same node address.
    /// Old groups keep their own Arc until their deferred paint has finished.
    pub(crate) fn collect(&self, nodes: &[Node], vcx: ValueCx<'_>) {
        fn walk(nodes: &[Node], out: &mut MaskDefinitions, vcx: ValueCx<'_>) {
            for n in nodes {
                let Node::Element(e) = n else { continue };
                let tag = e.tag.to_ascii_lowercase();
                if (tag == "mask" || tag == "clippath")
                    && let Some(id) = e.attr("id")
                {
                    let mut markup = String::new();
                    let ids = crate::resources::svg_document::ClipIds::for_document(e);
                    for c in &e.children {
                        if let Node::Element(el) = c {
                            crate::resources::svg_document::serialize::write_in(
                                el,
                                &mut markup,
                                &ids,
                                vcx,
                            );
                        }
                    }
                    out.0.insert(
                        id.to_string(),
                        MaskDefinition {
                            markup,
                            alpha: tag == "mask" && e.style.used().mask_type_alpha == Some(true),
                        },
                    );
                }
                if tag == "filter"
                    && let Some(id) = e.attr("id")
                {
                    let mut markup = String::new();
                    crate::resources::svg_document::serialize::write_element(e, &mut markup, vcx);
                    out.0.insert(
                        format!("filter:{id}"),
                        MaskDefinition {
                            markup,
                            alpha: false,
                        },
                    );
                }
                walk(&e.children, out, vcx);
            }
        }
        let mut definitions = MaskDefinitions::default();
        walk(nodes, &mut definitions, vcx);
        *self.definitions.borrow_mut() = Arc::new(definitions);
        self.prepared.set(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::state::DocState;
    use crate::style::css::Media;

    fn nodes(alpha: bool, fill: &str, doc: &std::rc::Rc<DocState>) -> Vec<Node> {
        let mode = if alpha { "alpha" } else { "luminance" };
        crate::dom::parse_current(
            &format!(
                "<svg><mask id='m' style='mask-type:{mode}'><rect width='20' height='20' fill='{fill}'/></mask><clipPath id='c'><rect width='10' height='20'/></clipPath><filter id='f'><feFlood flood-color='{fill}'/></filter></svg>"
            ),
            "",
            Media::default(),
            doc,
        )
    }

    #[test]
    fn delayed_masks_keep_markup_and_alpha_after_other_documents_and_rebuilds() {
        let doc = DocState::new(Media::default());
        let vcx = doc.values();
        let a = MaskResources::default();
        a.collect(&nodes(true, "red", &doc), vcx);
        let retained = a.definitions();
        let markup = retained.get("m").unwrap().markup.clone();
        assert!(retained.get("m").unwrap().alpha);
        assert!(!retained.get("c").unwrap().alpha);
        assert!(retained.get("filter:f").is_some());
        let b = MaskResources::default();
        b.collect(&nodes(false, "blue", &doc), vcx);
        assert!(!b.definitions().get("m").unwrap().alpha);
        assert_ne!(b.definitions().get("m").unwrap().markup, markup);
        // Rebuilding resources must not invalidate a retained group, regardless of count.
        for _ in 0..1002 {
            a.collect(&[], vcx);
        }
        assert!(a.definitions().get("m").is_none());
        assert_eq!(retained.get("m").unwrap().markup, markup);
        assert!(retained.get("m").unwrap().alpha);
    }

    #[test]
    fn rebuild_reads_changes_at_the_same_node_address() {
        fn recolor(nodes: &mut [Node]) {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "rect" {
                        e.attrs.retain(|(k, _)| k != "fill");
                        e.attrs.push(("fill".into(), "blue".into()));
                    }
                    recolor(&mut e.children);
                }
            }
        }
        let doc = DocState::new(Media::default());
        let vcx = doc.values();
        let context = MaskResources::default();
        let mut nodes = nodes(false, "red", &doc);
        let address = nodes.as_ptr();
        context.collect(&nodes, vcx);
        let before = context.definitions();
        recolor(&mut nodes);
        assert_eq!(nodes.as_ptr(), address);
        context.collect(&nodes, vcx);
        assert_ne!(
            before.get("m").unwrap().markup,
            context.definitions().get("m").unwrap().markup
        );
    }
}
