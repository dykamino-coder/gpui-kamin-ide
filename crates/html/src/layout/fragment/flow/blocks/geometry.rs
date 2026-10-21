//! Возможности разрыва блочной стопки по разложенным border box её коробок.
//!
//! Граница соседей-блоков — точка класса A (css-break-3 §4.3): `(need, from)` — низ
//! border box предыдущего и верх следующего. Поля между ними примыкают к разрыву и
//! усекаются (§5.2; Blink `discard_margins` после непринуждённого разрыва): копия
//! продолжается с верха следующей коробки. `break-before/after` — принудительный разрыв
//! на этой границе (§3.1). Граница, закрытая `avoid*` (css-break-4 §4.3 правило 1), — запрет:
//! сплошной диапазон от последней точки внутри предыдущего до начала следующего, и край
//! фрагментаинера в нём уводит разрыв раньше (Blink `EarlyBreak`,
//! `kBreakAppealViolatingBreakAvoid`). Рамка с отбивкой у начального края и монолиты —
//! сплошные диапазоны (§4.1); нижняя рамка с отбивкой без зазора держит примыкающее к ней
//! содержимое (класс C есть только при ненулевом зазоре, css-break-4 §possible-breaks).

use super::BlockBox;
use crate::layout::fragment::flow::FlowGeometry;
use crate::tolerance::COORD_EPS;

/// Точки разрыва стопки до слияния со строками.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockBreaks {
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) forced: Vec<f32>,
    pub(crate) solid: Vec<(f32, f32)>,
    /// Диапазоны, которым нужны все точки и монолиты (вместе со строками):
    /// в порядке обхода, вложенные раньше объемлющих.
    pending: Vec<Pending>,
}

/// Измеренные возможности разрыва копии: точки класса A `(need, from)`,
/// принудительные разрывы и сплошные диапазоны — вход стопки (`Kid`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MeasuredBreaks {
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) forced: Vec<f32>,
    pub(crate) solid: Vec<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pending {
    /// Граница под запретом: предыдущий ребёнок `[top, bottom)`, верх
    /// содержимого контейнера `floor`, начало следующего `next`.
    Avoid {
        top: f32,
        bottom: f32,
        floor: f32,
        next: f32,
    },
    /// Нижняя рамка с отбивкой `[edge, bottom)` коробки с верхом `top`;
    /// `gapless` — последний ребёнок кончается ровно на ней.
    Tail {
        top: f32,
        edge: f32,
        bottom: f32,
        gapless: bool,
    },
}

impl BlockBox {
    /// Точки разрыва стопки, корень которой кончается на `bottom`; `at(id)` —
    /// разложенный border box коробки `(верх, низ)` в координатах потока.
    /// Корень без записи стоит с нуля.
    pub(crate) fn breaks(
        &self,
        bottom: f32,
        at: &dyn Fn(u64) -> Option<(f32, f32)>,
    ) -> BlockBreaks {
        let mut out = BlockBreaks::default();
        let top = at(self.id).map_or(0.0, |b| b.0);
        walk(self, top, bottom, at, &mut out);
        out
    }
}

fn walk(
    b: &BlockBox,
    top: f32,
    bottom: f32,
    at: &dyn Fn(u64) -> Option<(f32, f32)>,
    out: &mut BlockBreaks,
) {
    if b.head > 0.0 {
        out.solid.push((top, top + b.head));
    }
    let floor = top + b.head;
    let mut prev: Option<(&super::BlockKid, f32, f32)> = None;
    let mut last: Option<f32> = None;
    // Конечное имя страницы последнего именованного соседа.
    let mut page_prev: Option<&str> = None;
    for k in &b.kids {
        let Some((kt, kb)) = at(k.id) else {
            // Коробка без записи раскладки: её места нет, границы вокруг неё
            // не строятся.
            prev = None;
            last = None;
            continue;
        };
        let renamed = matches!((page_prev, &k.page), (Some(a), Some((b, _))) if a != b);
        if let Some((_, end)) = &k.page {
            page_prev = Some(end);
        }
        if let Some((p, pt, pb)) = prev.filter(|_| k.adjoins) {
            out.cuts.push((pb, kt));
            if p.force_after || k.force_before || renamed {
                out.forced.push(pb);
            } else if p.avoid_after || k.avoid_before {
                out.pending.push(Pending::Avoid {
                    top: pt,
                    bottom: pb,
                    floor,
                    next: kt,
                });
            }
        }
        if k.monolith {
            out.solid.push((kt, kb));
        } else if let Some(inner) = &k.inner {
            walk(inner, kt, kb, at, out);
        }
        prev = Some((k, kt, kb));
        last = Some(kb);
    }
    if b.end_margin
        && b.tail <= 0.0
        && let Some(lb) = last
        && bottom > lb + COORD_EPS
    {
        out.cuts.push((lb, bottom));
    }
    if b.tail > 0.0 {
        let edge = bottom - b.tail;
        out.pending.push(Pending::Tail {
            top,
            edge,
            bottom,
            gapless: last.is_none_or(|lb| (edge - lb).abs() < 0.01),
        });
    }
}

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
                Pending::Tail {
                    top,
                    edge,
                    bottom,
                    gapless,
                } => {
                    let glue = if gapless {
                        g.solid
                            .iter()
                            .filter(|&&(a, e)| a > top + 0.01 && (e - edge).abs() < 0.01)
                            .map(|&(a, _)| a)
                            .fold(edge, f32::min)
                    } else {
                        edge
                    };
                    g.solid.push((glue, bottom));
                }
            }
        }
        g.cuts.retain(|&(need, _)| need < end - 0.01);
        g.normalize();
        let mut forced = self.forced;
        forced.retain(|&f| f > 0.01 && f < end - 0.01);
        forced.sort_by(f32::total_cmp);
        forced.dedup_by(|a, b| (*a - *b).abs() < COORD_EPS);
        g.solid.retain(|&(a, _)| a < end - 0.01);
        for r in g.solid.iter_mut() {
            r.1 = r.1.min(end);
        }
        MeasuredBreaks {
            cuts: g.cuts,
            forced,
            solid: g.solid,
        }
    }
}
