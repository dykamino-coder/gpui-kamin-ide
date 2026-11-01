//! Декорации делимых коробок отдельно от монолитов содержимого (css-break-3 §4.4).

use super::{BlockBox, Laid};

impl BlockBox {
    /// Рамка и padding по измеренным border box; монолитные потомки не
    /// раскрываются в BlockBox и сохраняют собственное правило переполнения.
    pub(crate) fn decorations(&self, laid: &Laid, out: &mut Vec<(f32, f32)>) {
        if let Some((top, bottom)) = laid.at(self.id) {
            // Если вся коробка состоит из декораций, сохраняем допустимое
            // переполнение неделимой пустой коробки (§4.4).
            let content = bottom - top > self.head + self.tail + 0.01;
            if content && self.head > 0.0 {
                out.push((top, top + self.head));
            }
            if content && self.tail > 0.0 {
                out.push((bottom - self.tail, bottom));
            }
        }
        for flow in self.table.iter().flat_map(|t| t.flows()) {
            flow.decorations(laid, out);
        }
        let kids = self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items));
        let grids = self.grid.iter().flat_map(|g| &g.items);
        for inner in kids
            .map(|k| &k.inner)
            .chain(grids.map(|k| &k.inner))
            .flatten()
        {
            inner.decorations(laid, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::flow::blocks::BlockKid;

    #[test]
    fn empty_padding_box_retains_its_overflow_policy() {
        let b = BlockBox {
            id: 1,
            head: 200.,
            ..Default::default()
        };
        let mut ranges = Vec::new();
        b.decorations(&Laid::of_boxes(&[(1, 0., 200.)]), &mut ranges);
        assert!(ranges.is_empty());
    }

    #[test]
    fn nested_decorations_use_their_own_measured_edges() {
        let b = BlockBox {
            id: 1,
            head: 20.,
            kids: vec![BlockKid {
                id: 2,
                inner: Some(BlockBox {
                    id: 2,
                    head: 5.,
                    tail: 3.,
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        let laid = Laid::of_boxes(&[(1, 0., 60.), (2, 20., 50.)]);
        let mut ranges = Vec::new();
        b.decorations(&laid, &mut ranges);
        assert_eq!(ranges, [(0., 20.), (20., 25.), (47., 50.)]);
    }
}
