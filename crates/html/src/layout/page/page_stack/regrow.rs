//! Рост flex-потоков по плану: каждая ширина сохраняет свои ограничения решателя.
use super::PageStack;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid};
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
            let mut grows = Vec::new();
            for f in plan.iter().filter(|f| f.kid == index) {
                if let Some(own) = self.continuations.get_mut(&(index, f.copy)) {
                    let Some(m) = self.measured.get(&(index, f.copy)) else {
                        continue;
                    };
                    let local = growths(blocks, &m.kid, &own.laid, f, heights[f.col]);
                    dirty |= own.push_many(local);
                } else {
                    grows.extend(growths(blocks, &kids[index], &r.laid, f, heights[f.col]));
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

fn growths(
    blocks: &BlockBox,
    k: &Kid,
    laid: &Laid,
    f: &Frag,
    height: f32,
) -> Vec<(BlockBox, f32, f32, bool)> {
    let end = f.from + f.h;
    let edge = f.from + height - f.y;
    let mut grows =
        crate::layout::fragment::push::flex_flow::growths(blocks, laid, f.from, end, edge);
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
    grows
}
