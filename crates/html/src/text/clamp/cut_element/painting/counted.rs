//! Counted line-clamp cuts and the end edges retained by intersecting boxes.

use super::ClampCut;
use super::ClampEntry;

impl ClampCut {
    pub(crate) fn counted_cut(
        &self,
        mut cut: Option<f32>,
        entries: &[ClampEntry],
        blocks: &[super::ClampBox],
        split: &dyn Fn(&ClampEntry) -> Vec<(f32, f32)>,
        sizes: &mut Vec<(u64, f32)>,
    ) -> (Option<f32>, Option<(u32, usize)>, bool) {
        let mut num_para: Option<(u32, usize)> = None;
        let mut by_count = false;
        if let Some(limit) = self.limit.filter(|n| *n > 0) {
            // (верх строки, низ строки, номер абзаца, номер строки в абзаце)
            let mut marks: Vec<(f32, f32, u32, usize)> = vec![];
            for e in entries.iter().filter(|e| e.line > 0.0 && !e.skip_count) {
                let Some(seq) = e.seq else { continue };
                let h = f32::from(e.bounds.size.height);
                if h <= 0.0 {
                    continue;
                }
                for (i, (a, b)) in split(e).into_iter().enumerate() {
                    marks.push((a, b, seq, i + 1));
                }
            }
            marks.sort_by(|a, b| a.0.total_cmp(&b.0));
            match marks.get(limit as usize - 1).copied() {
                Some((_, bottom, seq, k)) if cut.is_none_or(|c| bottom <= c + 0.5) => {
                    // Остаток СВОЕГО абзаца — знак нужен, потолок нет: абзац
                    // укоротит бюджет.
                    let own_rest = entries.iter().any(|e| {
                        e.line > 0.0
                            && e.seq == Some(seq)
                            && f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height)
                                > bottom + 0.5
                    });
                    // Другое содержимое за точкой: чужой абзац, выходящий за
                    // неё (в том числе несчитаемый), или коробка, начатая после.
                    let follows = entries.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        let h = f32::from(e.bounds.size.height);
                        h > 0.0
                            && if e.line > 0.0 {
                                e.seq != Some(seq) && y0 + h > bottom + 0.5
                            } else {
                                y0 >= bottom - 0.5
                            }
                    });
                    if own_rest || follows || entries.iter().any(|e| e.clamped.is_some()) {
                        num_para = Some((seq, k));
                    }
                    by_count = true;
                    // A paragraph that got its line budget and still runs
                    // past the point cannot clamp itself (the word-row path
                    // of a paragraph with atomic inlines): the box is cut at
                    // the point instead (`line-clamp-with-abspos-024`).
                    let unclamped = own_rest
                        && entries
                            .iter()
                            .any(|e| e.seq == Some(seq) && e.clamped.is_some());
                    if follows || unclamped {
                        // Коробки, содержащие точку, фрагментированы в ней и
                        // уносят свои нижние рамку и паддинг (§5.3); коробка с
                        // заданной высотой не фрагментируется — видна целиком.
                        // The boxes fragmented at the point are the
                        // ancestors of its paragraph in box-tree order.
                        let chain: Vec<&super::ClampBox> = blocks
                            .iter()
                            .filter(|b| {
                                !b.fixed
                                    && b.y0 < bottom
                                    && b.span.is_some_and(|(a, z)| (a..z).contains(&seq))
                            })
                            .collect();
                        let floor = blocks
                            .iter()
                            .filter(|b| b.fixed && b.y0 < bottom && bottom < b.y1)
                            .map(|b| b.flow_end())
                            .fold(bottom, f32::max);
                        cut = Some(super::chain_end(bottom, &chain).max(floor));
                        *sizes = super::chain_sizes(bottom, &chain);
                    }
                }
                // `max-height` теснее N строк — дальше как в авто-режиме.
                Some(_) => {}
                None => {
                    if self.max_h.is_none() {
                        // Строк меньше предела — среза нет.
                        cut = None;
                    }
                }
            }
        }
        (cut, num_para, by_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Bounds, point, px, size};

    fn paragraph(seq: u32, top: f32) -> ClampEntry {
        ClampEntry {
            bounds: Bounds {
                origin: point(px(0.0), px(top)),
                size: size(px(100.0), px(20.0)),
            },
            line: 20.0,
            skip_count: false,
            fixed_height: false,
            bp_after: 0.0,
            end_margin: 0.0,
            min_end: 0.0,
            capped: false,
            node: 0,
            seq: Some(seq),
            clamped: None,
            empty: false,
            span: None,
            bt_before: 0.0,
            positioned: false,
        }
    }

    #[test]
    fn counted_cut_contains_the_intersecting_float_end_margin() {
        let counter = ClampCut::new(0, Default::default(), Some(1), None);
        let entries = [paragraph(1, 0.0), paragraph(2, 20.0)];
        let mut blocks = [super::super::ClampBox {
            y0: 0.0,
            y1: 200.0,
            fixed: true,
            bp: 0.0,
            margin: 5.0,
            min_end: 0.0,
            node: 3,
            span: None,
        }];
        let split = |e: &ClampEntry| {
            let top = f32::from(e.bounds.origin.y);
            vec![(top, top + 20.0)]
        };
        let mut sizes = Vec::new();
        assert_eq!(
            counter.counted_cut(None, &entries, &blocks, &split, &mut sizes),
            (Some(205.0), Some((1, 1)), true)
        );
        // A negative margin cannot clip the float's visible border box.
        blocks[0].margin = -5.0;
        assert_eq!(
            counter
                .counted_cut(None, &entries, &blocks, &split, &mut sizes)
                .0,
            Some(200.0)
        );
    }
}
