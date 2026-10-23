//! Слияние точек блочной стопки со строками и прочими измеренными потоками
//! (`BlockBreaks::finish`): диапазоны, которым нужны все точки и монолиты, закрываются
//! после слияния — запрет `avoid*` от последней законной точки внутри предыдущего ребёнка
//! (Blink `early_break_`), верхняя и нижняя рамка с отбивкой с примыкающим без зазора
//! содержимым (css-break-4 §possible-breaks, класс C).

use super::geometry::{BlockBreaks, MeasuredBreaks, Pending};
use crate::layout::fragment::flow::FlowGeometry;
use crate::tolerance::COORD_EPS;

impl BlockBreaks {
    /// Слить со строками и прочими измеренными потоками (`lines`) и
    /// закрыть диапазоны, которым нужны все точки; всё — в пределах
    /// `[0, end)`.
    pub(crate) fn finish(self, lines: FlowGeometry, end: f32) -> MeasuredBreaks {
        let mut g = FlowGeometry {
            cuts: self.cuts,
            solid: self.solid,
            floats: Vec::new(),
        };
        g.cuts.extend(lines.cuts);
        g.solid.extend(lines.solid);
        for p in self.pending {
            match p {
                Pending::Avoid {
                    top,
                    bottom,
                    floor,
                    next,
                } => {
                    // Последняя законная точка ВНУТРИ предыдущего ребёнка, без
                    // неё — его начало (Blink `early_break_`). От верха
                    // содержимого диапазон не начинается: там это был бы
                    // разрыв перед контейнером, и его решает уровень выше.
                    let open = g
                        .cuts
                        .iter()
                        .map(|&(need, _)| need)
                        .filter(|&n| n > top + 0.01 && n < bottom - 0.01)
                        .fold(top, f32::max);
                    if open > floor + 0.01 {
                        g.solid.push((open, next + 0.05));
                    }
                }
                Pending::Head { top, edge } => {
                    let end = g
                        .solid
                        .iter()
                        .filter(|&&(a, _)| (a - edge).abs() < 0.01)
                        .map(|&(_, e)| e)
                        .fold(None::<f32>, |m, e| Some(m.map_or(e, |x| x.max(e))));
                    if let Some(end) = end {
                        g.solid.push((top, end));
                    }
                }
                Pending::Tail {
                    top,
                    edge,
                    from,
                    bottom,
                    gapless,
                } => {
                    let glue = if gapless {
                        g.solid
                            .iter()
                            .filter(|&&(a, e)| a > top + 0.01 && (e - edge).abs() < 0.01)
                            .map(|&(a, _)| a)
                            .fold(from, f32::min)
                    } else {
                        from
                    };
                    g.solid.push((glue, bottom));
                }
            }
        }
        g.cuts.retain(|&(need, _)| need < end - 0.01);
        g.normalize();
        let mut forced = self.forced;
        // Принудительный разрыв перед коробкой нулевой высоты в самом конце
        // потока тоже разрыв: после него есть содержимое (css-break-3 §3.1),
        // и оно начинает следующий фрагментаинер.
        forced.retain(|&f| f > 0.01 && f < end + 0.01);
        forced.sort_by(f32::total_cmp);
        forced.dedup_by(|a, b| (*a - *b).abs() < COORD_EPS);
        g.solid.retain(|&(a, _)| a < end - 0.01);
        for r in g.solid.iter_mut() {
            r.1 = r.1.min(end);
        }
        let mut avoid = self.avoid;
        avoid.retain(|&(a, _)| a < end - 0.01);
        MeasuredBreaks {
            cuts: g.cuts,
            forced,
            solid: g.solid,
            avoid,
            keep: self.keep,
            sections: self.sections,
        }
    }
}
