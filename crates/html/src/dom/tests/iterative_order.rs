//! Порядок template/shadow и наследование между соседними областями DOM.

use super::*;
use crate::budget::admission::Admission;
use crate::style::values::value::Color;

#[test]
fn shadow_slots_and_inert_templates_keep_order_and_inheritance() {
    let parsed = parse(
        "<div id=host style='color:green'><template shadowrootmode=open>
         <span id=s1>one</span><slot></slot><span id=s2>two</span>
         </template><p id=l1>light one</p><p id=l2>light two</p>
         <template><p id=inert>hidden</p></template></div>
         <p id=after style='color:blue'>after</p>",
        "",
    );
    crate::style::cascade::materialize::materialize(
        &parsed.nodes,
        &std::rc::Rc::new(crate::style::computed::Computed::default()),
        parsed.values(),
    );
    let admission = Admission::default();
    let elements: Vec<_> = crate::dom::traversal::nodes(&admission, &parsed.nodes, |_| true)
        .map(Result::unwrap)
        .filter_map(|n| match n {
            Node::Element(e) => e.attr("id").map(|id| (id, e)),
            _ => None,
        })
        .collect();
    assert_eq!(
        elements.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        ["host", "s1", "l1", "l2", "s2", "after"]
    );
    for (_, e) in &elements[..5] {
        assert_eq!(
            e.style.computed().color,
            Color::parse("green", parsed.values())
        );
    }
    assert_eq!(
        elements[5].1.style.computed().color,
        Color::parse("blue", parsed.values())
    );
}

#[test]
fn source_template_modes_preserve_depth_first_sibling_order() {
    use crate::dom::source_walk::{SourceWalk, Templates};
    use markup5ever_rcdom::NodeData;
    let input = crate::dom::input::Admitted::parse(
        "<div id=host><template id=first shadowrootmode=open>
         <b id=shadow></b><template id=inner><i id=inert></i></template>
         </template><template id=second shadowrootmode=open><i id=unused></i>
         </template><p id=light></p></div><p id=after></p>",
        false,
    )
    .unwrap();
    let ids = |templates| {
        let mut walk =
            SourceWalk::new(&input.dom.document, templates, Admission::default()).unwrap();
        let mut out = Vec::new();
        while let Some(node) = walk.try_next().unwrap() {
            if let NodeData::Element { attrs, .. } = &node.data {
                for attr in attrs.borrow().iter().filter(|a| &*a.name.local == "id") {
                    out.push(attr.value.to_string());
                }
            }
        }
        out
    };
    assert_eq!(
        ids(Templates::None),
        ["host", "first", "second", "light", "after"]
    );
    assert_eq!(
        ids(Templates::Active),
        [
            "host", "first", "shadow", "inner", "second", "light", "after"
        ]
    );
    assert_eq!(
        ids(Templates::All),
        [
            "host", "first", "shadow", "inner", "inert", "second", "unused", "light", "after"
        ]
    );
}
