//! Типы раскладки: display, списки промежутков, flex/grid-выравнивание, размещение, auto-flow.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Display {
    Block,
    /// `display: grid-lanes` — раскладка лунками (CSS Grid 3).
    GridLanes,
    /// `display: contents` — коробки нет, дети идут в поток родителя.
    Contents,
    /// `display: list-item` — блок с маркером.
    ListItem,
    /// `display: table-row` — ряд ячеек.
    TableRow,
    /// `display: table-row-group` и родня: обёртка над рядами. По раскладке
    /// это блок, но сборщик таблицы обязан заглянуть внутрь за рядами — от
    /// обычного блока его тем и надо отличать.
    TableRowGroup,
    /// `display: table` — контейнер табличной раскладки.
    Table,
    /// `display: inline-table` — та же таблица, но стоящая В СТРОКЕ.
    InlineTable,
    /// `display: table-cell` — ячейка.
    TableCell,
    /// `display: inline-grid`.
    InlineGrid,
    /// `inline-block`: коробка со своими размерами, но стоящая В СТРОКЕ.
    /// Отдельный вариант нужен потому, что раньше он схлопывался в `Block` и
    /// два таких элемента вставали друг под друга вместо одной строки.
    InlineBlock,
    Flex,
    InlineFlex,
    Grid,
    None,
}

/// Список значений линейки промежутков (css-gaps-1 §lists): ведущие значения,
/// тело `repeat(auto, …)` и хвостовые. Без авто-повтора список ЦИКЛИТСЯ по
/// промежуткам («repeat beginning from the first item in values»); с ним
/// ведущие идут от первого промежутка, хвостовые — от последнего, а тело
/// заполняет середину по кругу.
#[derive(Clone, Debug, PartialEq)]
pub struct GapList<T> {
    pub lead: Vec<T>,
    pub auto: Vec<T>,
    pub tail: Vec<T>,
}

impl<T: Copy> GapList<T> {
    pub fn single(v: T) -> Self {
        GapList {
            lead: vec![v],
            auto: vec![],
            tail: vec![],
        }
    }

    /// Первое значение — им живёт многоколонник, знающий одну линейку.
    pub fn first(&self) -> Option<T> {
        self.lead
            .first()
            .or(self.auto.first())
            .or(self.tail.first())
            .copied()
    }

    /// Больше одного значения или авто-повтор: скаляра недостаточно.
    pub fn is_plural(&self) -> bool {
        !self.auto.is_empty() || self.lead.len() + self.tail.len() > 1
    }

    pub fn any(&self, f: impl Fn(&T) -> bool) -> bool {
        self.lead.iter().chain(&self.auto).chain(&self.tail).any(f)
    }

    pub fn map<U: Copy>(&self, f: impl Fn(&T) -> U) -> GapList<U> {
        GapList {
            lead: self.lead.iter().map(&f).collect(),
            auto: self.auto.iter().map(&f).collect(),
            tail: self.tail.iter().map(&f).collect(),
        }
    }

    /// Значение промежутка `k` из `n` (§value-assignment).
    pub fn at(&self, k: usize, n: usize) -> Option<T> {
        if self.auto.is_empty() {
            let m = self.lead.len() + self.tail.len();
            if m == 0 {
                return None;
            }
            let i = k % m;
            return Some(if i < self.lead.len() {
                self.lead[i]
            } else {
                self.tail[i - self.lead.len()]
            });
        }
        if k < self.lead.len() {
            return Some(self.lead[k]);
        }
        let tail_from = n.saturating_sub(self.tail.len()).max(self.lead.len());
        if k >= tail_from {
            return self
                .tail
                .get(k - tail_from)
                .copied()
                .or(self.auto.first().copied());
        }
        Some(self.auto[(k - self.lead.len()) % self.auto.len()])
    }
}

/// Втяжка конца линейки (css-gaps-1 §inset): длина, доля ширины
/// пересекающего зазора или `overlap-join` — дотянуть до дальнего края
/// поперечной линейки.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GapInset {
    Len(Len),
    OverlapJoin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexDir {
    Row,
    RowReverse,
    Col,
    ColReverse,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
    /// css-anchor-position-1 §anchor-center: центр по якорю по умолчанию в
    /// пределах inset-modified containing block; без якоря или не у
    /// абсолюта — как `center` (так его и видит раскладка, `apply::to_items`).
    AnchorCenter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Justify {
    Start,
    Center,
    End,
    /// `left`/`right` — физические стороны; поперёк строчной оси (гибкая
    /// колонка) ведут себя как `start` (css-align-3 §5.2).
    Left,
    Right,
    /// `start`/`end` — оси ПИСЬМА, а не гибкой раскладки: при `row-reverse`
    /// они смотрят в другую сторону, чем `flex-start`/`flex-end`.
    WmStart,
    WmEnd,
    Between,
    Around,
    /// `space-evenly` — равные промежутки И по краям; у `space-around` края
    /// вдвое уже, поэтому свести их в одно значение нельзя.
    Evenly,
    Stretch,
}

/// Куда класть элемент в сетке: номер линии, число дорожек или «сама реши».
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Auto,
    Line(i16),
    Span(u16),
}

/// `grid-auto-flow`: в какую сторону раскладываются неразмещённые элементы.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AutoFlow {
    Row,
    Col,
    RowDense,
    ColDense,
}
