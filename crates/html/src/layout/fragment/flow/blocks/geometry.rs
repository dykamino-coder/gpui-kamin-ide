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
//! Таблица — своими подписями и рядами (`table::geometry`).

use super::stack::walk_kids;
use super::table::TableSections;
use super::{BlockBox, Laid, Pushed};
use crate::tolerance::COORD_EPS;

/// Точки разрыва стопки до слияния со строками.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockBreaks {
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) forced: Vec<f32>,
    pub(crate) solid: Vec<(f32, f32)>,
    /// Диапазоны, которым нужны все точки и монолиты (вместе со строками):
    /// в порядке обхода, вложенные раньше объемлющих.
    pub(super) pending: Vec<Pending>,
    /// Диапазоны запрета, достижимые только переносом (`MeasuredBreaks::avoid`).
    pub(crate) avoid: Vec<(f32, f32)>,
    /// Пожелания break-inside: avoid: перенос до начала, с пустого фрагмента допускается разрыв.
    pub(crate) keep: Vec<(f32, f32)>,
    /// Секции повтора, если корень стопки — таблица.
    pub(super) sections: Option<TableSections>,
}

/// Измеренные возможности разрыва копии: точки класса A `(need, from)`,
/// принудительные разрывы и сплошные диапазоны — вход стопки (`Kid`).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MeasuredBreaks {
    pub(crate) cuts: Vec<(f32, f32)>,
    pub(crate) forced: Vec<f32>,
    pub(crate) solid: Vec<(f32, f32)>,
    /// Диапазоны под `break-*: avoid`, край фрагментаинера в которых сам
    /// разрыв не уводит: в начало диапазона уходит лишь разрыв, перенесённый
    /// к его концу монолитом (`Kid::avoid`).
    pub(crate) avoid: Vec<(f32, f32)>,
    /// Пожелания break-inside: avoid: перенос до начала, с пустого фрагмента допускается разрыв.
    pub(crate) keep: Vec<(f32, f32)>,
    /// Разложенные секции повтора таблицы-корня (css-tables-3
    /// §repeated-headers).
    pub(crate) sections: Option<TableSections>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Pending {
    /// Граница под запретом: предыдущий ребёнок `[top, bottom)`, верх
    /// содержимого контейнера `floor`, начало следующего `next`.
    Avoid {
        top: f32,
        bottom: f32,
        floor: f32,
        next: f32,
    },
    /// Верхняя рамка с отбивкой `[top, edge)`, к которой без зазора
    /// примыкает содержимое: между ними нет точки разрыва (класс C — только
    /// при ненулевом зазоре, css-break-4 §possible-breaks), и сплошные
    /// диапазоны, начатые на `edge`, тянут её за собой.
    Head { top: f32, edge: f32 },
    /// Нижняя рамка с отбивкой коробки с верхом `top`, кончающейся на
    /// `bottom`: сплошная от `from`; `gapless` — последний ребёнок кончается
    /// ровно на `edge`, и сплошные диапазоны, кончающиеся там, приклеены к
    /// ней.
    Tail {
        top: f32,
        edge: f32,
        from: f32,
        bottom: f32,
        gapless: bool,
    },
}

impl BlockBox {
    /// Точки разрыва стопки, корень которой кончается на `bottom`; `at(id)` —
    /// разложенный border box коробки `(верх, низ)` в координатах потока.
    /// Корень без записи стоит с нуля.
    pub(crate) fn breaks(&self, bottom: f32, laid: &Laid) -> BlockBreaks {
        let mut out = BlockBreaks::default();
        let top = laid.at(self.id).map_or(0.0, |b| b.0);
        walk(self, top, bottom, laid, &mut out);
        out.sections = self.table.as_ref().and_then(|t| t.sections(laid));
        out
    }

    /// Самая внешняя коробка стопки, чей разложенный border box начинается
    /// ровно в `a` (`laid` — разложенная копия в координатах потока): перед ней
    /// встаёт распорка роста. Коробка, накрывающая `a`, — спуск в её стопку;
    /// монолит и коробка без своей стопки спуска не дают. В сетке — ряд,
    /// начинающийся в `a` (`GridBox::pushed_at`).
    pub(crate) fn pushed_at(&self, a: f32, laid: &Laid) -> Option<Pushed> {
        if let Some(t) = &self.table {
            return t.pushed_at(a, laid);
        }
        if let Some(g) = &self.grid {
            return g.pushed_at(self.id, a, laid);
        }
        if let Some(f) = &self.flex {
            return f.pushed_at(self.id, a, laid);
        }
        for k in &self.kids {
            let Some((top, bottom)) = laid.at(k.id) else {
                continue;
            };
            if (top - a).abs() < COORD_EPS {
                return Some(Pushed::Box(k.id));
            }
            if top < a
                && a < bottom - COORD_EPS
                && let Some(found) = k.inner.as_ref().and_then(|b| b.pushed_at(a, laid))
            {
                return Some(found);
            }
        }
        laid.lines
            .get(&self.id)?
            .iter()
            .find(|p| (p.at - a).abs() < COORD_EPS)
            .cloned()
            .map(Pushed::Line)
    }
}

/// Точки стопки `b`, стоящей в `[top, bottom)`.
pub(super) fn walk(b: &BlockBox, top: f32, bottom: f32, laid: &Laid, out: &mut BlockBreaks) {
    if b.inside_avoid {
        out.keep.push((top, bottom));
    }
    if let Some(flow) = laid.flows.get(&b.id) {
        out.cuts.extend_from_slice(&flow.cuts);
        let lines = laid.content.get(&b.id).map_or(&[][..], |l| l.as_slice());
        for &(top, bottom) in &flow.solid {
            let inside: Vec<_> = lines
                .iter()
                .filter(|l| l.top >= top - COORD_EPS && l.bottom <= bottom + COORD_EPS)
                .collect();
            if inside.len() < 2 {
                out.solid.push((top, bottom));
                continue;
            }
            // Строки, сведённые `orphans`/`widows` (`FlowGeometry::keep_lines`),
            // — пожелание, а не монолит: правило 3 css-break-3 §4.4 снимается,
            // если иначе содержимое не помещается во фрагментаинер (Blink
            // `kBreakAppealViolatingOrphansAndWidows`).
            out.keep.push((top, bottom));
            for pair in inside.windows(2) {
                out.cuts.push((pair[0].bottom, pair[1].top));
            }
            out.solid
                .extend(inside.iter().map(|l| (l.top.max(top), l.bottom.min(bottom))));
        }
    }
    if let Some(t) = &b.table {
        t.walk(top, laid, out);
        return;
    }
    if b.head > 0.0 {
        out.solid.push((top, top + b.head));
    }
    let floor = top + b.head;
    let last = match (&b.grid, &b.flex) {
        (Some(g), _) => g.breaks(b.id, floor, laid, out),
        (_, Some(f)) => f.breaks(b.id, floor, laid, out),
        _ => walk_kids(b, floor, laid, out),
    };
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
            from: edge,
            bottom,
            gapless: last.is_none_or(|lb| (edge - lb).abs() < 0.01),
        });
    }
}
