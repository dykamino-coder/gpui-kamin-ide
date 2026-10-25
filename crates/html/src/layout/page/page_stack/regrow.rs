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
mod tests {
    use super::*;
    use crate::layout::fragment::flow::{LinePush, blocks::Pushed};
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn unforced_line_break_consumes_unused_fragment_extent_once() {
        let blocks = BlockBox {
            id: 1,
            ..Default::default()
        };
        let growth = Rc::new(RefCell::new(Vec::new()));
        let mut laid = Laid::default();
        laid.lines.insert(
            1,
            vec![LinePush {
                at: 80.0,
                line: 2,
                growth: growth.clone(),
            }],
        );
        let kid = Kid {
            h: 160.0,
            cuts: vec![(80.0, 80.0)],
            ..Default::default()
        };
        let mut frag = Frag {
            kid: 0,
            copy: 0,
            col: 0,
            y: 0.0,
            from: 0.0,
            h: 80.0,
            head: 0.0,
            foot: 0.0,
        };
        let grows = growths(&blocks, &kid, &laid, &frag, 100.0);
        assert_eq!(grows.len(), 1);
        assert_eq!(grows[0].amount, 20.0);
        assert!(!grows[0].forced);
        let Pushed::Line(line) = &grows[0].target else {
            panic!("line boundary")
        };
        assert!(line.grow(grows[0].amount));
        assert_eq!(*growth.borrow(), vec![0.0, 0.0, 20.0]);
        // После публикации занятого места новый разрыв стоит у края.
        frag.h = 100.0;
        assert!(growths(&blocks, &kid, &laid, &frag, 100.0).is_empty());
    }
}
