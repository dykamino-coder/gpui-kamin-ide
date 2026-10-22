//! Рост flex-потоков по плану листов с пересборкой содержимого и позиционированных слоёв.
use super::PageStack;
use crate::layout::fragment::types::{Frag, Kid};
use crate::tolerance::COORD_EPS;
impl PageStack {
    pub(super) fn regrow_flex(&mut self, kids: &[Kid], plan: &[Frag], heights: &[f32]) -> bool {
        let mut dirty = false;
        for (index, kid) in self.kids.iter_mut().enumerate() {
            let (Some(r), Some(w)) = (kid.regrow.as_mut(), kid.watched.as_ref()) else {
                continue;
            };
            let Some(blocks) = w.blocks() else { continue };
            let k = &kids[index];
            let mut grows = Vec::new();
            for f in plan.iter().filter(|f| f.kid == index) {
                let end = f.from + f.h;
                let edge = f.from + heights[f.col] - f.y;
                grows.extend(crate::layout::fragment::push::flex_flow::growths(
                    blocks, &r.laid, f.from, end, edge,
                ));
                let solid = k.solid.iter().any(|p| (p.0 - end).abs() < COORD_EPS);
                let forced = k.forced.iter().any(|v| (v - end).abs() < COORD_EPS);
                if (solid || forced) && end < k.h - COORD_EPS && edge > end + COORD_EPS {
                    let at = k
                        .cuts
                        .iter()
                        .find(|p| (p.0 - end).abs() < COORD_EPS)
                        .map_or(end, |p| p.1);
                    grows.push((blocks.clone(), at, edge - end, forced && !solid));
                }
            }
            if r.push_many(grows) {
                (kid.el, kid.frags) = r.rebuild(kid.frags.len());
                w.reset();
                dirty = true;
            }
        }
        dirty
    }
}
