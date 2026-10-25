//! Края колонки следуют order, края ряда — линиям в блочном потоке.
use super::*;
use crate::layout::fragment::flow::blocks::{FlexBox, FlexFrame, FlexLine};

fn scenario(column: bool, lines: &[&[u64]], order: &[u64]) -> (BlockBox, Laid) {
    let mut root = BlockBox {
        id: 10,
        ..Default::default()
    };
    root.flex = Some(FlexBox {
        items: order
            .iter()
            .map(|&id| BlockKid {
                id,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    });
    let mut laid = Laid::default();
    let lines = lines
        .iter()
        .enumerate()
        .map(|(index, members)| {
            for (position, &id) in members.iter().enumerate() {
                let top = if column { position } else { index } as f32 * 40.0;
                laid.boxes.insert(id, (top, top + 30.0));
            }
            FlexLine {
                index,
                start: index as f32 * 40.0,
                end: index as f32 * 40.0 + 30.0,
                items: members.to_vec(),
                order: order
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| members.contains(id))
                    .map(|(i, &id)| (i as u32, Some(id)))
                    .collect(),
            }
        })
        .collect();
    laid.flexes.insert(10, FlexFrame { column, lines });
    (root, laid)
}

#[test]
fn column_reverse_propagates_semantic_first_and_last() {
    let (mut root, laid) = scenario(true, &[&[2, 1]], &[1, 2]);
    let items = &mut root.flex.as_mut().unwrap().items;
    items[0].force_before = true;
    items[0].avoid_after = true;
    items[1].avoid_before = true;
    items[1].force_after = true;
    assert_eq!(root.edge(false, &laid), (true, false));
    assert_eq!(root.edge(true, &laid), (true, false));
}

#[test]
fn wrap_reverse_propagates_block_flow_edge_lines() {
    let (mut root, laid) = scenario(false, &[&[4, 3], &[2, 1]], &[1, 2, 3, 4]);
    let items = &mut root.flex.as_mut().unwrap().items;
    items[0].force_after = true;
    items[1].avoid_after = true;
    items[2].force_before = true;
    items[3].avoid_before = true;
    assert_eq!(root.edge(false, &laid), (true, true));
    assert_eq!(root.edge(true, &laid), (true, true));
}

#[test]
fn row_propagation_depends_on_lines_not_their_first_ordered_item() {
    // §10: break-before всех items переносится на линию, затем с крайней
    // линии на контейнер. Blink ApplyReversals предшествует этому переносу;
    // Gecko обходит строки от StartmostLine в блочном направлении.
    let physical: &[&[u64]] = &[&[6, 5], &[4, 3], &[2, 1]];
    for order in [[1, 2, 3, 4, 5, 6], [6, 5, 4, 3, 2, 1]] {
        let (mut root, laid) = scenario(false, physical, &order);
        for item in &mut root.flex.as_mut().unwrap().items {
            item.avoid_before = item.id == 2;
            item.force_before = item.id == 5;
        }
        assert_eq!(root.edge(false, &laid), (true, false));
    }
}

#[test]
fn reversed_row_line_break_moves_container_after_preceding_content() {
    use crate::layout::fragment::{flow::FlowGeometry, plan, types::Kid};
    let (mut root, mut laid) = scenario(false, &[&[6, 5], &[4, 3], &[2, 1]], &[1, 2, 3, 4, 5, 6]);
    for item in &mut root.flex.as_mut().unwrap().items {
        item.avoid_before = item.id == 2;
        item.force_before = item.id == 5;
    }
    let frame = laid.flexes.get_mut(&10).unwrap();
    for (i, (line, (start, end))) in frame
        .lines
        .iter_mut()
        .zip([(0.0, 50.0), (50.0, 100.0), (100.0, 200.0)])
        .enumerate()
    {
        // Идентификатор native-линии не задаёт блочный порядок.
        line.index = 2 - i;
        line.start = start;
        line.end = end;
        for &id in &line.items {
            laid.boxes.insert(id, (start, end));
        }
    }
    let measured = root
        .breaks(200.0, &laid)
        .finish(FlowGeometry::default(), 200.0);
    assert!(
        measured.forced.is_empty(),
        "forced первой строки выходит наружу"
    );
    assert_eq!(
        measured.avoid.len(),
        1,
        "avoid нижней строки остаётся внутри"
    );
    let before = root.edge(false, &laid);
    let kids = [
        Kid {
            h: 50.0,
            over: 50.0,
            monolith: true,
            ..Default::default()
        },
        Kid {
            h: 200.0,
            over: 200.0,
            force_before: before.0,
            avoid_before: before.1,
            cuts: measured.cuts,
            solid: measured.solid,
            avoid: measured.avoid,
            keep: measured.keep,
            ..Default::default()
        },
    ];
    let (_, _, fragments) = plan::fill(&kids, 100.0, plan::Space::columns(4));
    let first = fragments.iter().find(|f| f.kid == 1).unwrap();
    assert_eq!((first.col, first.y, first.from), (1, 0.0, 0.0));
    assert_eq!(
        fragments
            .iter()
            .filter(|f| f.kid == 1)
            .map(|f| f.h)
            .sum::<f32>(),
        200.0
    );
}

#[test]
fn reverse_columns_each_propagate_their_semantic_edges() {
    let (mut root, laid) = scenario(true, &[&[4, 3], &[2, 1]], &[1, 2, 3, 4]);
    let items = &mut root.flex.as_mut().unwrap().items;
    items[0].avoid_before = true;
    items[2].force_before = true;
    items[1].avoid_after = true;
    items[3].force_after = true;
    assert_eq!(root.edge(false, &laid), (true, true));
    assert_eq!(root.edge(true, &laid), (true, true));
}

#[test]
fn order_modified_edges_are_independent_of_node_ids_and_coordinates() {
    let (mut root, laid) = scenario(true, &[&[2, 4, 1, 3]], &[3, 1, 4, 2]);
    let items = &mut root.flex.as_mut().unwrap().items;
    items[0].force_before = true;
    items[0].avoid_after = true;
    items[3].avoid_before = true;
    items[3].force_after = true;
    assert_eq!(root.edge(false, &laid), (true, false));
    assert_eq!(root.edge(true, &laid), (true, false));
}

#[test]
fn anonymous_column_edge_does_not_propagate_the_next_element() {
    let (mut root, mut laid) = scenario(true, &[&[2, 1]], &[1, 2]);
    root.flex.as_mut().unwrap().items[0].force_before = true;
    laid.flexes.get_mut(&10).unwrap().lines[0]
        .order
        .insert(0, (0, None));
    assert_eq!(root.edge(false, &laid), (false, false));
}

#[test]
fn css_order_precedes_reverse_for_forced_and_avoid_propagation() {
    use crate::dom::{Node, parse};
    let tree = parse(
        "<div><div style='order:2'></div><div style='order:-1'></div><div style='order:1'></div><div style='order:-1'></div></div>",
        "",
    );
    fn div(node: &Node) -> Option<&crate::dom::Element> {
        let Node::Element(e) = node else { return None };
        if e.tag == "div" {
            return Some(e);
        }
        e.children.iter().find_map(div)
    }
    let parent = tree.iter().find_map(div).expect("flex root");
    let ids: Vec<_> = crate::layout::block::reorder::reorder(parent.children.to_vec())
        .into_iter()
        .filter_map(|n| match n {
            Node::Element(e) => Some(e.node_id),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 4);
    let physical: Vec<_> = ids.iter().copied().rev().collect();
    for column in [false, true] {
        let (mut root, laid) = scenario(column, &[&physical], &ids);
        let items = &mut root.flex.as_mut().unwrap().items;
        items[0].force_before = true;
        items[0].avoid_before = true;
        items[3].force_after = true;
        items[3].avoid_after = true;
        assert_eq!(root.edge(false, &laid), (true, true));
        assert_eq!(root.edge(true, &laid), (true, true));
    }
}
