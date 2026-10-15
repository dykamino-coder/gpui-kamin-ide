//! MathML Core В§2.1.5 and В§3.8: namespace-aware cascade defaults and hints.

use crate::style::css::{Rule, Selector, parse_decls};
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn is_mathml(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. }
        if &*name.ns == "http://www.w3.org/1998/Math/MathML")
}

/// Keep hints below author rules, including layered declarations. Construct
/// declarations directly: an attribute value cannot inject extra properties.
pub(super) fn hints(handle: &Handle, attrs: &[(String, String)]) -> Vec<Rule> {
    if !is_mathml(handle) {
        return vec![];
    }
    let mut rule = Rule {
        sel: Selector::parse("*").expect("universal selector"),
        decls: parse_decls(""),
        order: 0,
        origin: 1,
        layer: vec![],
    };
    for (name, value) in attrs {
        let property = match name.as_str() {
            "mathcolor" if valid_color(value) => "color",
            "mathbackground" if valid_color(value) => "background-color",
            "mathsize" if valid_size(value) => "font-size",
            _ => continue,
        };
        rule.decls.insert(property.to_string(), value.clone());
    }
    vec![rule]
}

fn valid_color(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("currentcolor")
        || crate::style::values::value::Color::parse(value).is_some()
}

fn valid_size(value: &str) -> bool {
    use crate::style::values::value::Len;
    let value = value.trim();
    if value.parse::<f32>().is_ok_and(|n| n != 0.0) {
        return false;
    }
    matches!(Len::parse(value), Some(Len::Px(v) | Len::Em(v) | Len::Pct(v)
        | Len::Ex(v) | Len::Ch(v) | Len::Ic(v) | Len::Lh(v) | Len::Vw(v) | Len::Vh(v)) if v >= 0.0)
}

/// UA visibility is resolved before tree construction, and remains overridable
/// by author CSS. Unlike deleting alternatives, this preserves the cascade.
pub(super) fn defaults(handle: &Handle, index: usize) -> Vec<Rule> {
    let parent = handle.parent.take();
    let hides = index > 1 && parent.as_ref().and_then(std::rc::Weak::upgrade)
        .is_some_and(|parent| is_mathml(&parent) && matches!(&parent.data,
            NodeData::Element { name, .. } if matches!(name.local.as_ref(), "semantics" | "maction")));
    handle.parent.set(parent);
    if !hides {
        return vec![];
    }
    vec![Rule {
        sel: Selector::parse("semantics > :not(:first-child)").expect("UA selector"),
        decls: parse_decls("display: none"),
        order: usize::MAX,
        origin: 0,
        layer: vec![],
    }]
}

#[cfg(test)]
mod tests {
    use crate::dom::{Element, Node, parse};
    use crate::style::values::value::{Color, Len};
    fn find<'a>(nodes: &'a [Node], id: &str) -> &'a Element {
        fn search<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.attr("id") == Some(id) => Some(e),
                Node::Element(e) => search(&e.children, id),
                _ => None,
            })
        }
        search(nodes, id).expect("element")
    }
    #[test]
    fn hints_are_namespace_scoped_and_below_author_css() {
        let nodes = parse(
            r#"<span id="html" mathcolor="red" mathsize="200%"></span>
            <math><mtext id="hint" mathcolor="red" mathsize="200%">x</mtext>
            <mtext id="css" mathcolor="red" mathbackground="red" mathsize="200%"
            style="color: green; background-color: blue; font-size: 30px">x</mtext></math>"#,
            "",
        );
        assert_eq!(find(&nodes, "html").style.font_size, None);
        assert_eq!(find(&nodes, "html").style.color, None);
        assert_eq!(find(&nodes, "hint").style.color, Color::parse("red"));
        assert_eq!(find(&nodes, "hint").style.font_size, Some(Len::Pct(2.0)));
        assert_eq!(find(&nodes, "css").style.font_size, Some(Len::Px(30.0)));
        assert_eq!(find(&nodes, "css").style.color, Color::parse("green"));
    }
    #[test]
    fn invalid_mathsize_and_injected_declarations_are_rejected() {
        let nodes = parse(
            r#"<math><mtext id="bad" mathsize="large" mathcolor="red; display:none">x</mtext></math>"#,
            "",
        );
        let e = find(&nodes, "bad");
        assert_eq!(e.style.font_size, None);
        assert_eq!(e.style.color, None);
    }
    #[test]
    fn semantics_ua_hiding_remains_overridable() {
        let nodes = parse(
            r#"<math><semantics><mi id="first">x</mi>
          <annotation id="hidden">hidden</annotation>
          <annotation id="shown" style="display: inline">shown</annotation>
          </semantics></math>"#,
            "",
        );
        assert_eq!(find(&nodes, "first").tag, "mi");
        assert_eq!(find(&nodes, "shown").tag, "annotation");
        fn contains(nodes: &[Node], id: &str) -> bool {
            nodes.iter().any(|n|matches!(n,Node::Element(e) if e.attr("id")==Some(id)||contains(&e.children,id)))
        }
        assert!(!contains(&nodes, "hidden"));
    }
}
