//! Монолитность наблюдаемых коробок: решатель не растит их из-за внутренних разрывов.
use super::Watched;
use crate::layout::fragment::flow::blocks::BlockBox;
use std::collections::HashSet;

impl Watched {
    pub(super) fn monolithic(&self, id: u64) -> bool {
        self.monolithic_ids_of().contains(&id)
    }

    pub(super) fn monolithic_ids_of(&self) -> &HashSet<u64> {
        self.monolithic_ids.get_or_init(|| {
            let mut ids = HashSet::new();
            if let Some(blocks) = &self.blocks {
                collect(blocks, &mut ids);
            }
            ids
        })
    }
}

fn collect(blocks: &BlockBox, ids: &mut HashSet<u64>) {
    let ordinary = blocks
        .kids
        .iter()
        .chain(blocks.flex.iter().flat_map(|f| &f.items))
        .map(|k| (k.id, k.monolith, &k.inner));
    let grid = blocks
        .grid
        .iter()
        .flat_map(|g| &g.items)
        .map(|k| (k.id, k.monolith, &k.inner));
    for (id, monolith, inner) in ordinary.chain(grid) {
        if monolith {
            ids.insert(id);
        }
        if let Some(inner) = inner {
            collect(inner, ids);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::flow::blocks::{BlockKid, FlexBox};

    #[test]
    fn copies_preserve_monolithic_classification() {
        let rec = Watched::for_blocks(BlockBox {
            id: 1,
            flex: Some(FlexBox {
                items: vec![BlockKid {
                    id: 2,
                    monolith: true,
                    inner: Some(BlockBox {
                        id: 2,
                        kids: vec![BlockKid {
                            id: 3,
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        for copy in [
            rec.clone(),
            rec.nested(),
            rec.constraints_only(),
            rec.continuation_recorder(1),
        ] {
            assert!(copy.monolithic(2));
            assert!(!copy.monolithic(1));
            assert!(!copy.monolithic(3));
        }
    }
}
