//! HTML option labels and the menu's intrinsic sizing content.

use crate::dom::{Element, Node};

#[derive(Default)]
pub(super) struct Options {
    pub labels: Vec<String>,
    pub selected: usize,
}

impl Options {
    pub fn from_element(e: &Element) -> Self {
        let mut options = Self::default();
        options.collect(&e.children);
        options
    }

    fn collect(&mut self, nodes: &[Node]) {
        for node in nodes {
            let Node::Element(e) = node else { continue };
            if e.tag == "optgroup" {
                self.collect(&e.children);
            } else if e.tag == "option" {
                let mut text = String::new();
                crate::render::gather_text_public(&e.children, &mut text);
                let label = e
                    .attr("label")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| text.split_ascii_whitespace().collect::<Vec<_>>().join(" "));
                if e.attr("selected").is_some() {
                    self.selected = self.labels.len();
                }
                self.labels.push(label);
            }
        }
    }

    pub fn selected_label(&self) -> &str {
        self.labels
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::Computed;

    fn element(tag: &str, attrs: &[(&str, &str)], children: Vec<Node>) -> Element {
        Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: tag.into(),
            style: Computed::default(),
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            inline: false,
        }
    }

    #[test]
    fn labels_keep_unselected_options_and_optgroups() {
        let e = element(
            "select",
            &[],
            vec![
                Node::Element(element("option", &[], vec![Node::Text("  a\n b  ".into())])),
                Node::Element(element(
                    "optgroup",
                    &[],
                    vec![Node::Element(element(
                        "option",
                        &[("label", "wide label"), ("selected", "")],
                        vec![Node::Text("ignored text".into())],
                    ))],
                )),
            ],
        );
        let options = Options::from_element(&e);
        assert_eq!(options.labels, ["a b", "wide label"]);
        assert_eq!(options.selected_label(), "wide label");
    }
}
