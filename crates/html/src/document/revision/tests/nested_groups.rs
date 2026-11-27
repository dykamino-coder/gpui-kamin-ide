//! CSS Nesting 1 sections 3.3 and 5: nested groups retain cascade semantics.

use crate::document::Document;
use crate::dom::{Element, Node};
use crate::style::values::value::Color;

fn element<'a>(nodes: &'a [Node], id: &str) -> &'a Element {
    fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(el) if el.attrs.iter().any(|(k, v)| k == "id" && v == id) => Some(el),
            Node::Element(el) => find(&el.children, id),
            _ => None,
        })
    }
    find(nodes, id).unwrap()
}

#[test]
fn nested_groups_match_chrome_across_widths_and_directions() {
    for width in [375., 800., 1440.] {
        for direction in ["ltr", "rtl"] {
            let html = format!(
                r#"<!doctype html><html dir={direction}><style>
                .media {{ color:red; @MEDIA(min-width:800px) {{ color:green }} }}
                .normal {{ @layer a {{ color:green }} color:blue }}
                .important {{ @layer a {{ color:green!important }} color:blue!important }}
                .restore {{ @layer a {{ color:red; @layer b {{ color:green }} color:blue }} }}
                .recover {{ color:green; @mediafoo(all) {{ color:red }} }}
                </style><div id=media class=media>media</div>
                <div id=normal class=normal>normal</div>
                <div id=important class=important>important</div>
                <div id=restore class=restore>restore</div>
                <div id=recover class=recover>recover</div></html>"#
            );
            let doc = Document::builder(&html, crate::style::ua::BROWSER_CSS)
                .viewport((width, 900.))
                .build()
                .unwrap();
            for (id, color) in [
                ("media", if width < 800. { "red" } else { "green" }),
                ("normal", "blue"),
                ("important", "green"),
                ("restore", "blue"),
                ("recover", "green"),
            ] {
                assert_eq!(
                    element(doc.nodes(), id).style.cascaded().inh.ui.color,
                    Color::parse(color, doc.state.values.values()),
                    "{id}, {width}, {direction}"
                );
            }
        }
    }
}
