//! Края и paint-ресурс пустого блока: slice целой коробки либо декорации каждого clone.

use crate::dom::{Element, Node};
use crate::layout::fragment::flow::independent_copy;
use crate::layout::options::RenderOpts;
use crate::layout::page::page_stack::LeafPaint;
use crate::style::computed::{BoxStyle, Display};
use gpui::{IntoElement, ParentElement};
use std::rc::Rc;

pub(super) fn paint_leaf(
    opts: &RenderOpts,
    root: &BoxStyle,
    group: &[Node],
    padding: (f32, f32, f32),
    height: f32,
    percentages: &crate::layout::fragment::flow::PagePercentages,
) -> Option<LeafPaint> {
    let [Node::Element(e)] = group else {
        return None;
    };
    if padding.2 != 0.0 || !eligible(e) {
        return None;
    }
    let (mut opts, root, group) = (opts.clone(), root.clone(), group.to_vec());
    opts.frag.watch = None;
    let context = percentages.context(e.node_id);
    let (geometry_context, geometry_opts) = (context.clone(), opts.clone());
    Some(LeafPaint {
        geometry: Rc::new(move |space| geometry_context.leaf(space, &geometry_opts).0),
        build: Rc::new(move |space, f| {
            // Только paint готовой leaf-геометрии: ни детей, ни copy/remeasure.
            let frame = super::frame::wrap(padding, height, false, None);
            let (leaf, style) = context.leaf(space, &opts);
            let mut nodes = group.clone();
            let [Node::Element(c)] = nodes.as_mut_slice() else {
                unreachable!()
            };
            let used = c.style.used_mut();
            used.height = style.height;
            used.min_height = style.min_height;
            used.max_height = style.max_height;
            if leaf.edges.clone {
                let edges = leaf
                    .edges
                    .fit_clone(f.h, (leaf.content_block - f.from).max(0.0));
                *c = crate::layout::fragment::clone::clone_fragment(
                    c,
                    leaf.edges.start,
                    leaf.edges.end,
                    f.from,
                    f.h,
                    (f.h - edges.sum()).max(0.0),
                );
            }
            frame
                .children(opts.cx.contracts.build.blocks(&nodes, &root, &opts))
                .into_any_element()
        }),
    })
}

pub(super) fn eligible(e: &Element) -> bool {
    let c = e.style.used();
    let mut flow = e.clone();
    flow.style.used_mut().bdb_clone = false;
    // IFC и прочие FC принадлежат своим child consumers.
    if !e.children.is_empty()
        || !independent_copy(&flow)
        || c.float.is_some_and(|side| side != 0)
        || !matches!(c.display, None | Some(Display::Block))
        || crate::layout::box_kind::replaced_tag(e)
        || c.aspect_ratio.is_some()
        || c.aspect_ratio_auto.is_some()
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accepts(body: &str) -> bool {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let tree = crate::dom::parse(body, "");
        eligible(find(&tree).expect("authored div"))
    }

    #[test]
    fn empty_block_accepts_containing_block_constraints_and_decorations() {
        for css in [
            "height:2500px",
            "min-height:2500px",
            "max-height:20px",
            "height:2500px;margin-top:-20px",
            "height:2500px;padding-inline:5px",
            "height:50%",
            "min-height:50%",
            "max-height:50%",
            "height:2500px;padding:5px;border:2px solid;box-decoration-break:clone",
            "height:2500px;padding:5px;border:2px solid;box-sizing:border-box",
        ] {
            assert!(accepts(&format!("<div style='{css}'></div>")), "{css}");
        }
    }

    #[test]
    fn child_flows_and_edges_require_their_own_consumers() {
        for body in [
            "<div>text</div>",
            "<div><div></div></div>",
            "<div style='display:flex'></div>",
            "<div style='display:grid'></div>",
            "<div style='float:left'></div>",
            "<div style='position:absolute'></div>",
            "<div style='writing-mode:vertical-rl'></div>",
            "<div style='aspect-ratio:2'></div>",
        ] {
            assert!(!accepts(body), "{body}");
        }
    }
}
