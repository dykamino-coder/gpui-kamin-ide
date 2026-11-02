//! Высоты вычисляются снизу вверх, позиции — сверху вниз без новых layout-деревьев.

use super::{Kind, Laid, STRUT_ID, Serial};

impl Serial {
    pub(super) fn project(&self, laid: &mut Laid) -> f32 {
        // Узлы записаны после детей; размер колонки не влияет на ширину IFC.
        let mut deltas = vec![0.; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            deltas[i] = match &node.kind {
                Kind::Leaf(leaf) => self.leaves[*leaf].total(),
                Kind::Column(kids) => kids.iter().map(|&k| deltas[k] + self.nodes[k].offset).sum(),
            };
        }
        let mut shifts = vec![0.; self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate().rev() {
            let shift = shifts[i] + node.offset;
            let top = node.bounds.0 + shift;
            let bottom = node.bounds.1 + shift + deltas[i];
            laid.boxes.insert(node.id, (top, bottom));
            if node.offset > 0. {
                laid.boxes
                    .insert(node.id ^ STRUT_ID, (top - node.offset, top));
            } else {
                laid.boxes.remove(&(node.id ^ STRUT_ID));
            }
            match &node.kind {
                Kind::Leaf(leaf) => {
                    let flow = self.leaves[*leaf].geometry(shift);
                    laid.content.insert(
                        node.id,
                        flow.inline
                            .iter()
                            .flat_map(|p| p.lines.iter().cloned())
                            .collect(),
                    );
                    laid.lines.insert(node.id, flow.line_pushes.clone());
                    laid.flows.insert(node.id, flow);
                }
                Kind::Column(kids) => {
                    let mut before = 0.;
                    for &k in kids {
                        shifts[k] = shift + before;
                        before += deltas[k] + self.nodes[k].offset;
                    }
                    if let Some(mut frame) = laid.flexes.get(&node.id).cloned() {
                        frame.lines[0].start =
                            self.nodes[kids[0]].bounds.0 + shift + self.nodes[kids[0]].offset;
                        frame.lines[0].end = bottom;
                        laid.flexes.insert(node.id, frame);
                    }
                }
            }
        }
        deltas.last().copied().unwrap_or_default()
    }
}
