//! UA-умолчания MathML: display, видимость и математическая глубина (math-depth).
use super::mathml::is_mathml;
use crate::style::css::{Rule, Selector, parse_decls};
use markup5ever_rcdom::{Handle, NodeData};

/// UA visibility is resolved before tree construction, and remains overridable
/// by author CSS. Unlike deleting alternatives, this preserves the cascade.
pub(super) fn defaults(handle: &Handle, index: usize) -> Vec<Rule> {
    let parent = handle.parent.take();
    let hides = index > 1 && parent.as_ref().and_then(std::rc::Weak::upgrade)
        .is_some_and(|parent| is_mathml(&parent) && matches!(&parent.data,
            NodeData::Element { name, .. } if matches!(name.local.as_ref(), "semantics" | "maction")));
    handle.parent.set(parent);
    let mut rules = vec![];
    if super::mathml_routing::enabled(handle) {
        let mut css = "display: block math";
        if let NodeData::Element { name, attrs, .. } = &handle.data {
            css = match name.local.as_ref() {
                "math"
                    if attrs.borrow().iter().any(|a| {
                        &*a.name.local == "display" && a.value.eq_ignore_ascii_case("block")
                    }) =>
                {
                    "display: block math"
                }
                "math" => "display: inline math",
                "mfrac" => "display: block math; padding-inline: 1px",
                "mphantom" => "display: block math; visibility: hidden",
                _ => css,
            };
        }
        rules.push(Rule {
            sel: Selector::parse("*").expect("UA selector"),
            decls: parse_decls(css),
            order: 0,
            origin: 0,
            layer: vec![],
        });
    }
    if super::mathml_routing::enabled(handle) {
        let mut decls = parse_decls("font-size: math");
        if let NodeData::Element { name, .. } = &handle.data
            && &*name.local == "math"
        {
            decls.merge(parse_decls(
                "math-style: compact; math-depth: 0; math-shift: normal",
            ));
            if let NodeData::Element { attrs, .. } = &handle.data
                && attrs
                    .borrow()
                    .iter()
                    .any(|a| &*a.name.local == "display" && a.value.eq_ignore_ascii_case("block"))
            {
                decls.merge(parse_decls("math-style: normal"));
            }
        }
        let parent = handle.parent.take();
        if let Some(parent) = parent
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
            .filter(is_mathml)
            && let NodeData::Element { name, .. } = &parent.data
        {
            let css = match name.local.as_ref() {
                "mfrac" => "math-depth: auto-add; math-style: compact",
                "mroot" if index > 1 => "math-depth: add(2); math-style: compact",
                "msub" | "msup" | "msubsup" | "mmultiscripts" | "munder" | "mover"
                | "munderover"
                    if index > 1 =>
                {
                    "math-depth: add(1); math-style: compact"
                }
                _ => "",
            };
            decls.merge(parse_decls(css));
        }
        handle.parent.set(parent);
        rules.push(Rule {
            sel: Selector::parse("*").expect("UA selector"),
            decls,
            order: 0,
            origin: 0,
            layer: vec![],
        });
    }
    // Core §4.1 UA sheet (Blink mathml.css:82): single-character identifiers
    // are italic by default; independent of the formatting context.
    if is_mathml(handle)
        && matches!(&handle.data, NodeData::Element { name, .. } if &*name.local == "mi")
    {
        rules.push(Rule {
            sel: Selector::parse("*").expect("UA selector"),
            decls: parse_decls("text-transform: math-auto"),
            order: 0,
            origin: 0,
            layer: vec![],
        });
    }
    if hides {
        rules.push(Rule {
            sel: Selector::parse("semantics > :not(:first-child)").expect("UA selector"),
            decls: parse_decls("display: none"),
            order: usize::MAX,
            origin: 0,
            layer: vec![],
        });
    }
    rules
}
