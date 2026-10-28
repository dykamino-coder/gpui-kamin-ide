//! Рекордер главной копии ребёнка стопки страниц: строки — всегда, блочная стопка
//! (`flow::blocks`) — у группы из одной коробки с поддеревом обычных блоков
//! (`stacked_tree`) или таблицы из них (`table_tree`). Границы соседних блоков
//! записываются и при атомарном строчном содержимом: оно не отменяет разрыв
//! класса A между содержащими его блоками (css-break-3 §4.1, §4.3).

use crate::document::state::ValueCx;
use crate::dom::Node;
use crate::dom::whitespace::is_blank;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::flow::blocks::{BlockBox, flex_tree, stacked_tree, table_tree};
use crate::layout::fragment::paginate::watch::Watched;
use crate::style::computed::Position;
use std::rc::Rc;

pub(super) fn recorder(
    group: &[Node],
    n: &Node,
    vertical: Option<bool>,
    shaped: bool,
    measured: bool,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> Rc<Watched> {
    let single = group.iter().filter(|g| !is_blank(g)).count() == 1;
    match n {
        Node::Element(e)
            if single
                && (vertical.is_none() || measured)
                && !e.inline
                && !matches!(
                    e.style.used().position,
                    Some(Position::Absolute) | Some(Position::Fixed)
                )
                && (measured || stacked_tree(e) || (shaped && (table_tree(e) || flex_tree(e)))) =>
        {
            let logical = vertical.map(|rl| {
                crate::layout::fragment::flow::logical_copy(e, rl)
                    .expect("measured vertical copy retains its logical metadata")
            });
            let e = logical.as_ref().unwrap_or(e);
            let blocks = BlockBox::collect(e, cx, vcx);
            // Without block children the copy keeps its measured lines alone.
            if blocks.is_empty() && !measured {
                Watched::new(&[])
            } else {
                use crate::layout::fragment::types::StackAxis;
                let axis = match vertical {
                    None => StackAxis::Horizontal,
                    Some(false) => StackAxis::VerticalLr,
                    Some(true) => StackAxis::VerticalRl,
                };
                Watched::for_blocks_axis(blocks, axis, crate::layout::fragment::flow::flow_lines(e))
            }
        }
        _ => Watched::new(&[]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Element;
    use crate::layout::fragment::flow::blocks::Laid;

    fn authored(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|n| match n {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("authored block")
    }

    #[test]
    fn nested_atomic_inline_keeps_the_enclosing_block_recorder() {
        let tree = crate::dom::parse(
            "<div><div style='height:100px;clear:both'><div style='float:left;height:100px'></div><span>X<span style='display:inline-block;height:100%'></span><br>Y</span></div></div>",
            "",
        );
        let e = authored(&tree);
        assert!(stacked_tree(e));
        assert!(crate::layout::fragment::flow::regrow_copy(e));
        assert!(!crate::layout::fragment::push::measured_tree(e));
        let group = [Node::Element(e.clone())];
        let watched = recorder(
            &group,
            &group[0],
            None,
            false,
            false,
            ShapeCx::COLUMNS,
            tree.values(),
        );
        assert_eq!(watched.blocks().unwrap().kids.len(), 1);
    }

    #[test]
    fn atomic_inline_content_does_not_hide_forced_break_between_block_parents() {
        let tree = crate::dom::parse(
            "<div><div><span style='display:inline-block'>one</span></div><div style='break-before:page'><span style='display:inline-block'>two</span></div></div>",
            "",
        );
        let e = authored(&tree);
        let group = [Node::Element(e.clone())];
        assert!(!crate::layout::fragment::push::measured_tree(e));
        let watched = recorder(
            &group,
            &group[0],
            None,
            false,
            false,
            ShapeCx::COLUMNS,
            tree.values(),
        );
        let blocks = watched
            .blocks()
            .expect("block boundaries remain measurable");
        assert_eq!(blocks.kids.len(), 2);
        let laid = Laid::of_boxes(&[
            (e.node_id, 0., 100.),
            (blocks.kids[0].id, 0., 50.),
            (blocks.kids[1].id, 50., 100.),
        ]);
        assert_eq!(blocks.breaks(100., &laid).forced, vec![50.]);
    }
}
