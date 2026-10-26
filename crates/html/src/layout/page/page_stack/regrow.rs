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
            // По одному фрагменту за проход: рост сдвигает содержимое ниже, и
            // точки следующих фрагментов прежней раскладки устарели.
            let mut frags: Vec<_> = plan.iter().filter(|f| f.kid == index).collect();
            frags.sort_by_key(|f| f.copy);
            for f in frags {
                if let Some(own) = self.continuations.get_mut(&(index, f.copy)) {
                    let Some(m) = self.measured.get(&(index, f.copy)) else {
                        continue;
                    };
                    let mut local = growths(blocks, &m.kid, &own.laid, f, heights[f.col]);
                    local.extend(own.float_growths(
                        f.from,
                        f.from + heights[f.col] - f.y,
                        heights[f.col],
                    ));
                    dirty |= own.push_many(local);
                } else if grows.is_empty() {
                    grows = growths(blocks, &kids[index], &r.laid, f, heights[f.col]);
                    grows.extend(r.float_growths(
                        f.from,
                        f.from + heights[f.col] - f.y,
                        heights[f.col],
                    ));
                }
            }
            if r.push_many(grows) {
                (kid.el, kid.frags) = r.rebuild(kid.frags.len());
                w.reset();
                // Продолжения наследуют ограничения главной копии. После
                // роста её float floors старые копии имеют другую геометрию.
                self.continuations.retain(|&(owner, _), _| owner != index);
                dirty = true;
            }
        }
        dirty
    }
}

pub(super) fn growths(
    blocks: &BlockBox,
    k: &Kid,
    laid: &Laid,
    f: &Frag,
    height: f32,
) -> Vec<crate::layout::fragment::push::Growth> {
    let end = f.from + f.h;
    let edge = f.from + height - f.y;
    let mut grows =
        crate::layout::fragment::push::flex_flow::growths(blocks, laid, f.from, end, edge);
    let solid = k
        .solid
        .iter()
        .chain(&k.keep)
        .any(|p| (p.0 - end).abs() < COORD_EPS);
    let forced = k.forced.iter().any(|v| (v - end).abs() < COORD_EPS);
    // Class B между строками тоже занимает неиспользованный остаток
    // фрагментаинера (css-break-3 §5.3): его сохраняет рекордер абзаца.
    if end < k.h.max(k.over) - COORD_EPS && edge > end + COORD_EPS {
        let at = k
            .cuts
            .iter()
            .find(|p| (p.0 - end).abs() < COORD_EPS)
            .map_or(end, |p| p.1);
        let growth = crate::layout::fragment::push::Growth::at(
            blocks,
            laid,
            at,
            edge - end,
            forced && !solid,
        );
        grows.extend(growth.filter(|g| {
            solid
                || forced
                || matches!(
                    g.target,
                    crate::layout::fragment::flow::blocks::Pushed::Line(_)
                )
        }));
    }
    grows
}

#[cfg(test)]
mod tests;
