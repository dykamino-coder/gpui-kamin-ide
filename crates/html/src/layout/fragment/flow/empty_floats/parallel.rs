//! Параллельные float сохраняют собственные ограничения разрыва.
use crate::layout::fragment::flow::{FloatGeometry, FlowGeometry};

pub(crate) fn separate(flow: &FlowGeometry) -> Vec<FloatGeometry> {
    flow.floats.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Node;

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
        let flow = FlowGeometry {
            solid: vec![(0.0, 100.0)],
            floats: vec![FloatGeometry {
                id: float.node_id,
                anchor: 0.0,
                top: 0.0,
                bottom: 100.0,
                margins: [0.0; 2],
                first_break: None,
            }],
            ..Default::default()
        };
        assert_eq!(separate(&flow).len(), 1);
        assert_eq!(flow.solid, vec![(0.0, 100.0)]);
    }
}
