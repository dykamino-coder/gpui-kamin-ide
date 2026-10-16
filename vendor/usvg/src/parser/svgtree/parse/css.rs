//! SVG typed-tree css; source cascade remains in its resource scope.
pub(super) fn resolve_css<'a>(
    xml: &'a roxmltree::Document<'a>,
    style_sheet: Option<&'a str>,
) -> simplecss::StyleSheet<'a> {
    let mut sheet = simplecss::StyleSheet::new();

    // Injected style sheets do not override internal ones (we mimic the logic of rsvg-convert),
    // so we need to parse it first.
    if let Some(style_sheet) = style_sheet {
        sheet.parse_more(style_sheet);
    }

    for node in xml.descendants().filter(|n| n.has_tag_name("style")) {
        match node.attribute("type") {
            Some("text/css") => {}
            Some(_) => continue,
            None => {}
        }

        let text = match node.text() {
            Some(v) => v,
            None => continue,
        };

        sheet.parse_more(text);
    }

    sheet
}

pub(super) struct XmlNode<'a, 'input: 'a>(pub(super) roxmltree::Node<'a, 'input>);

impl simplecss::Element for XmlNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(XmlNode)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(XmlNode)
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag_name().name() == local_name
    }

    fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
        match self.0.attribute(local_name) {
            Some(value) => operator.matches(value),
            None => false,
        }
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
        match class {
            simplecss::PseudoClass::FirstChild => self.prev_sibling_element().is_none(),
            // TODO: lang
            _ => false, // Since we are querying a static SVG we can ignore other pseudo-classes.
        }
    }
}
