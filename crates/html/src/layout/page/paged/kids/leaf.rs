//! Paint-ресурс пустого блока: геометрия slice сохраняется, запас копий не строится.

use crate::dom::{Element, Node};
use crate::layout::fragment::flow::independent_copy;
use crate::layout::options::RenderOpts;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, px};
use std::rc::Rc;

pub(crate) type PaintLeaf = Rc<dyn Fn() -> AnyElement>;

pub(super) fn paint_leaf(
    opts: &RenderOpts,
    root: &BoxStyle,
    group: &[Node],
    padding: (f32, f32, f32),
    height: f32,
) -> Option<PaintLeaf> {
    let [Node::Element(e)] = group else {
        return None;
    };
    if padding.2 != 0.0 || !eligible(e) {
        return None;
    }
    let (mut opts, root, group) = (opts.clone(), root.clone(), group.to_vec());
    opts.frag.watch = None;
    Some(Rc::new(move || {
        // Это только paint slice уже измеренной leaf-геометрии (css-break-3 §5.4.1):
        // ни дочернего дерева, ни нового прохода copy/remeasure здесь нет.
        let mut frame = super::frame::wrap(padding, height, false, None);
        if let [Node::Element(e)] = group.as_slice()
            && matches!(e.tag.as_str(), "html" | "body")
            && matches!(e.style.used().height, Some(Len::Pct(_)))
        {
            frame = frame.h(px(height));
        }
        frame
            .children(opts.cx.contracts.build.blocks(&group, &root, &opts))
            .into_any_element()
    }))
}

fn eligible(e: &Element) -> bool {
    let c = e.style.used();
    // У leaf нет дочерних геометрических зависимостей. Border/padding принадлежат
    // отдельному алгоритму краёв, а IFC и прочие FC — своим child consumers.
    if !e.children.is_empty()
        || !independent_copy(e)
        || !matches!(c.display, None | Some(Display::Block))
        || crate::layout::box_kind::replaced_tag(e)
        || c.aspect_ratio.is_some()
        || c.aspect_ratio_auto.is_some()
    {
        return false;
    }
    let b = c.borders();
    [b.top, b.bottom, c.padding.top, c.padding.bottom]
        .iter()
        .all(|l| matches!(l, None | Some(Len::Px(0.0))))
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
    fn empty_block_accepts_constraints_without_percentage_dependencies() {
        for css in [
            "height:2500px",
            "min-height:2500px",
            "max-height:20px",
            "height:2500px;margin-top:-20px",
            "height:2500px;padding-inline:5px",
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
            "<div style='padding-top:1px'></div>",
            "<div style='border-bottom:1px solid'></div>",
            "<div style='float:left'></div>",
            "<div style='position:absolute'></div>",
            "<div style='writing-mode:vertical-rl'></div>",
            "<div style='box-decoration-break:clone'></div>",
            "<div style='aspect-ratio:2'></div>",
            // Процентный CB продолжения вводится в P04; MaxContent его не доказывает.
            "<div style='height:50%'></div>",
            "<div style='min-height:50%'></div>",
            "<div style='max-height:50%'></div>",
        ] {
            assert!(!accepts(body), "{body}");
        }
    }
}
