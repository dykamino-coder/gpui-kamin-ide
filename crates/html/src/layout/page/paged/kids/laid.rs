//! Подготовка независимой печатной копии без предварительной оценки геометрии.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::block::margins::{fragment_box, fragment_margins};
use crate::layout::fragment::flow::{empty_floats, independent_copy};
use crate::layout::fragment::line_shape::resolved_lengths;
use crate::style::computed::Computed;

pub(super) struct Group {
    pub(super) nodes: Vec<Node>,
    pub(super) margins: (f32, f32, f32),
    pub(super) blocks: bool,
    pub(super) float_extent: bool,
}

pub(super) fn group(
    nodes: &[Node],
    root: &Computed,
    width: f32,
    top: f32,
    vcx: ValueCx<'_>,
) -> Option<Group> {
    let [Node::Element(e)] = nodes else {
        return None;
    };
    let floated = e.style.float.is_some_and(|f| f != 0);
    let float_extent = empty_floats(e);
    let eligible = if floated {
        // ADR P-19 F2-1: только монолит не несёт собственного продолжения
        // строк. Поля float остаются внутри BandFlow и не схлопываются.
        let mut flow = e.clone();
        flow.style.float = None;
        crate::layout::fragment::clone::content_monolith_blockified(e) && independent_copy(&flow)
    } else {
        independent_copy(e) || float_extent
    };
    if !eligible {
        return None;
    }
    let e: Element = fragment_box(resolved_lengths(e, root, vcx), width, vcx);
    let (mt, mb) = fragment_margins(&e)?;
    Some(Group {
        nodes: vec![Node::Element(e)],
        blocks: !floated,
        float_extent,
        margins: if floated {
            (0.0, 0.0, 0.0)
        } else if top > 0.0 {
            (0.0, mb, 0.0)
        } else {
            (mt, mb, mt)
        },
    })
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn empty_page_grid_uses_native_group_without_estimated_shape() {
        let tree = crate::dom::parse(
            "<div style='display:grid;grid-template-rows:20px 30px'></div>",
            "",
        );
        let nodes = [Node::Element(authored(&tree).clone())];
        let p = group(&nodes, &Computed::default(), 120.0, 0.0, tree.values()).unwrap();
        assert!(p.blocks);
        let Node::Element(e) = &p.nodes[0] else {
            panic!("grid")
        };
        let blocks = crate::layout::fragment::flow::blocks::BlockBox::collect(
            e,
            crate::layout::fragment::ShapeCx::COLUMNS,
            tree.values(),
        );
        assert!(!blocks.is_empty());
        assert!(blocks.grid.as_ref().unwrap().items.is_empty());
    }

    fn authored(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored float")
    }

    #[test]
    fn monolithic_page_float_keeps_its_measured_margin_box() {
        let tree = crate::dom::parse(
            "<div style='float:left;overflow:scroll;margin-top:20%;margin-bottom:10%;width:40px'>float</div>",
            "",
        );
        let nodes = [Node::Element(authored(&tree).clone())];
        let p = group(&nodes, &Computed::default(), 120.0, 15.0, tree.values())
            .expect("native monolithic float");
        assert!(!p.blocks);
        assert_eq!(p.margins, (0.0, 0.0, 0.0));
        let Node::Element(e) = &p.nodes[0] else {
            panic!("float")
        };
        assert_eq!(e.style.float, Some(-1));
        assert_eq!(e.style.margin.top, Some(Len::Px(24.0)));
        assert_eq!(e.style.margin.bottom, Some(Len::Px(12.0)));
    }

    #[test]
    fn flowing_page_float_requires_its_own_continuation() {
        let tree = crate::dom::parse("<div style='float:left;width:20px'>flowing text</div>", "");
        let nodes = [Node::Element(authored(&tree).clone())];
        assert!(group(&nodes, &Computed::default(), 120.0, 0.0, tree.values()).is_none());
    }

    #[test]
    fn empty_page_floats_use_their_layout_extent_even_when_the_pair_wraps() {
        let tree = crate::dom::parse(
            "<div style='width:100px;padding:5px'><div style='float:left;width:70px;height:40px'></div><div style='float:right;width:70px;height:60px'></div></div>",
            "",
        );
        let nodes = [Node::Element(authored(&tree).clone())];
        let p = group(&nodes, &Computed::default(), 120.0, 0.0, tree.values()).unwrap();
        assert!(p.blocks && p.float_extent);
        let Node::Element(e) = &p.nodes[0] else {
            panic!("float host")
        };
        assert_eq!(e.style.width, Some(Len::Px(100.0)));
    }
}
