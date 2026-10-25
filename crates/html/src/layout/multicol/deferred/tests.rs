//! Отложенная раскладка сохраняет процентный CB при определении ширины колонки.
use super::*;
use crate::layout::fragment::types::StackAxis;
use crate::style::values::value::Len;

fn section(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "section" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("section")
}

#[test]
fn deferred_children_resolve_percentages_from_original_content_box() {
    let tree = crate::dom::parse(
        "<!doctype html><section style='height:400px;box-sizing:border-box;padding:20px;border:10px solid;column-count:2'><div style='height:50%;min-height:25%;max-height:75%'></div></section>",
        "",
    );
    let host = section(&tree);
    let mut opts = RenderOpts {
        text: Default::default(),
        viewport: (800.0, 600.0),
        normal_line_height: 1.2,
        cx: Default::default(),
        frag: Default::default(),
    };
    opts.cx.values.doc = crate::layout::cx::BuildDoc(tree.doc.clone());
    opts.cx.values.viewport = opts.viewport;
    crate::style::cascade::materialize::materialize(
        &tree,
        opts.root_style().style_rc(),
        opts.cx.vcx(),
    );
    let merged = crate::style::cascade::materialize::box_style(
        &opts.root_style(),
        &host.style,
        opts.cx.vcx(),
    );
    let recipe = Recipe::new(
        host,
        &merged,
        &opts,
        2,
        16.0,
        Some(100.0),
        StackAxis::Horizontal,
        None,
    )
    .expect("deferred measured tree");
    for width in [100.0, 160.0] {
        let resolved = recipe.resolved_host(width);
        let Node::Element(child) = &resolved.children[0] else {
            unreachable!()
        };
        assert_eq!(child.style.used().height, Some(Len::Px(170.0)));
        let measured = RefCell::default();
        let kids = laid::children(&resolved, &merged, opts.cx.vcx(), width, &measured)
            .expect("resolved measured tree");
        let child = kids[0].0.style.used();
        assert_eq!(child.height, Some(Len::Px(170.0)));
        assert_eq!(child.min_height, Some(Len::Px(85.0)));
        assert_eq!(child.max_height, Some(Len::Px(255.0)));
    }
    let Node::Element(original) = &host.children[0] else {
        unreachable!()
    };
    assert_eq!(original.style.used().height, Some(Len::Pct(0.5)));
}
