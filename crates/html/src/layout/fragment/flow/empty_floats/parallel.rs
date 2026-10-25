//! Пустые float сохраняют собственный запрет разрыва, а не запрет родителя.
use super::*;
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry};
use crate::tolerance::COORD_EPS;

pub(crate) fn separate(flow: &mut FlowGeometry, source: &Element) -> Vec<FloatGeometry> {
    fn monolithic_float(e: &Element, id: u64) -> bool {
        if e.node_id == id {
            return e.style.used().float.is_some_and(|f| f != 0)
                && crate::layout::fragment::clone::monolith_box(e);
        }
        children(&e.children).any(|n| match n {
            Node::Element(k) => monolithic_float(k, id),
            _ => false,
        })
    }
    let mut floats = Vec::new();
    for f in &flow.floats {
        floats.push(*f);
        if !monolithic_float(source, f.id) {
            continue;
        }
        let range = (f.top - f.margins[0], f.bottom + f.margins[1]);
        // BandFlow публикует ровно один такой диапазон для монолитного float.
        // Совпавший диапазон другого владельца остаётся в геометрии.
        if let Some(i) = flow
            .solid
            .iter()
            .position(|p| (p.0 - range.0).abs() < COORD_EPS && (p.1 - range.1).abs() < COORD_EPS)
        {
            flow.solid.remove(i);
        }
    }
    floats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_constraint_does_not_move_its_fixed_height_parent() {
        let tree = crate::dom::parse(
            "<div style='height:100px'><div style='float:left;height:100px;break-inside:avoid'></div></div>",
            "",
        );
        let source = super::super::tests::authored(&tree);
        let Node::Element(float) = &source.children[0] else {
            panic!("float")
        };
        let mut flow = FlowGeometry {
            solid: vec![(0.0, 100.0), (0.0, 100.0)],
            floats: vec![FloatGeometry {
                id: float.node_id,
                top: 0.0,
                bottom: 100.0,
                margins: [0.0; 2],
                first_break: None,
            }],
            ..Default::default()
        };
        assert_eq!(separate(&mut flow, source).len(), 1);
        assert_eq!(flow.solid, vec![(0.0, 100.0)]);
    }
}
