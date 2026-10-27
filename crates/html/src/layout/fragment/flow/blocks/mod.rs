//! Блочная стопка копии ребёнка фрагментации: какие коробки записывает рекордер раскладки
//! (`paginate::watch::Watched`) и какие свойства их границ читает фрагментация. Положения и
//! размеры берутся из РАЗЛОЖЕННЫХ border box и дорожек сетки таблиц (`laid`), а не из оценки
//! формы: из дерева — только то, что задаёт стиль (принудительные разрывы и запреты на
//! границах с переносом с крайних детей, монолитность, рамка и отбивка у краёв, корень
//! блочного контекста, устройство таблицы — `table`). Возможности разрыва класса A между
//! соседями-блоками (css-break-3 §4.3) и усечение полей на разрыве (§5.2) — в `geometry`.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;

mod collect;
pub(crate) mod leaf;
mod registry;
use collect::{collect, frame, push_kids};
mod column_avoid;
mod finish;
mod geometry;
mod parallel;
mod pushed;
mod stack;
pub(crate) use geometry::{BlockBreaks, MeasuredBreaks};
mod laid;
pub(crate) use laid::{Laid, TableFrame};
mod trees;
pub(crate) use trees::{plain_tree, stacked_tree, table_tree};
pub(crate) mod table;
use table::TableBox;
mod grid;
mod grid_breaks;
pub(crate) use grid::{GridBox, Pushed, grid_container};
mod grid_tree;
pub(crate) use grid_tree::grid_tree;
mod flex;
mod flex_breaks;
mod flex_edges;
mod flex_gaps;
mod flex_line_starts;
pub(crate) use flex_line_starts::pending as forced_line_start_pending;
mod flex_tree;
pub(crate) use flex::{FlexBox, FlexFrame, FlexLine, FlexPush, flex_container};
pub(crate) use flex_tree::flex_tree;
#[cfg(test)]
mod depth_tests;
#[cfg(test)]
mod flex_avoid_tests;
#[cfg(test)]
mod flex_column_tests;
#[cfg(test)]
mod flex_gap_tests;
#[cfg(test)]
mod flex_line_start_tests;
#[cfg(test)]
mod flex_overflow_tests;
#[cfg(test)]
mod flex_tests;
#[cfg(test)]
mod grid_flow_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod mixed_tests;
#[cfg(test)]
mod tests;

/// Номер распорки роста перед коробкой `id` — `id ^ STRUT_ID`
/// (`push::spacer::spacer_before`): рекордер стопки пишет и её, и её низ —
/// граница, на которой встаёт разрыв перед коробкой.
pub(crate) const STRUT_ID: u64 = 0x5350_4143_4552_0001;

/// Блочный контейнер стопки: его рамка с отбивкой у краёв и поточные
/// блочные дети в порядке потока.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockBox {
    pub(crate) id: u64,
    /// Собственные before/after force, before/after avoid до переноса с детей.
    pub(crate) own_breaks: (bool, bool, bool, bool),
    pub(crate) inside_avoid: bool,
    /// Обрезка блочного переполнения ограничивает продолжение размером коробки.
    pub(crate) clips_block: bool,
    /// Поточные крайние дети: строчный текст на краю останавливает перенос break.
    pub(crate) edge_kids: (Option<u64>, Option<u64>),
    /// Рамка и отбивка у начального края блочной оси: разрыв внутри них
    /// недопустим (Blink «avoid breaking inside block-start border»).
    pub(crate) head: f32,
    /// Рамка и отбивка у конечного края.
    pub(crate) tail: f32,
    /// Корень блочного контекста с `auto` высотой: нижнее поле последнего
    /// ребёнка входит в коробку (CSS 2.1 §8.3.1, §10.6.7) и усекается
    /// разрывом после ребёнка (css-break-3 §5.2).
    pub(crate) end_margin: bool,
    pub(crate) kids: Vec<BlockKid>,
    /// Устройство таблицы, если коробка — таблица (её обёртка): дети тогда
    /// пусты, границы дают подписи и ряды (`table`).
    pub(crate) table: Option<Box<TableBox>>,
    /// Элементы, если контейнер — сетка: детей стопкой она не кладёт.
    pub(crate) grid: Option<GridBox>,
    pub(crate) flex: Option<FlexBox>,
    /// Поток элементов flex-колонки: промежуток главной оси (`row-gap`) между
    /// соседями.
    pub(crate) flex_gap: Option<f32>,
}

/// Поточный блочный ребёнок контейнера.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockKid {
    pub(crate) id: u64,
    /// Граница с предыдущим ребёнком — граница соседей-блоков (класс A).
    /// Ложно, если между ними строчное содержимое или коробка без записи.
    pub(crate) adjoins: bool,
    /// Принудительные разрывы и запреты на краях с переносом с крайних
    /// потомков (css-break-3 §3.1 break propagation: `edge_break`,
    /// `edge_avoid`).
    pub(crate) force_before: bool,
    pub(crate) force_after: bool,
    pub(crate) avoid_before: bool,
    pub(crate) avoid_after: bool,
    /// Монолит (css-break-3 §4.1): разрыв внутри недопустим.
    pub(crate) monolith: bool,
    /// Своя блочная стопка, если ребёнок — блочный контейнер.
    pub(crate) inner: Option<BlockBox>,
    /// Начальное и конечное имя страницы коробки класса A у страниц
    /// (css-page-3 §using-named-pages п. 4): несовпадение конца предыдущего
    /// с началом следующего — принудительный разрыв на их границе.
    pub(crate) page: Option<(String, String)>,
}

impl BlockBox {
    /// Стопка копии `e` в условиях меры `cx`: у страниц (`paged`) — имена
    /// страниц, `viewport` решает единицы окна рамок (как у меры формы).
    pub(crate) fn collect(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> Self {
        collect(e, false, cx, vcx)
    }

    /// Номера узлов корня и всех коробок стопки (их раскладку пишет
    /// рекордер).
    pub(crate) fn ids(&self) -> Vec<u64> {
        let mut out = vec![self.id];
        self.push_ids(&mut out);
        out
    }

    /// Даже без элементов сетка имеет дорожки, которые наблюдает рекордер.
    pub(crate) fn is_empty(&self) -> bool {
        self.kids.is_empty()
            && self.table.is_none()
            && self.grid.is_none()
            && self.flex.as_ref().is_none_or(|f| f.items.is_empty())
    }

    /// Сетки стопки: номер контейнера и номера его элементов — дорожки рядов
    /// читаются у самого разложенного контейнера (`grid_tracks`).
    pub(crate) fn grids(&self, out: &mut Vec<(u64, Vec<u64>)>) {
        for flow in self.table.iter().flat_map(|t| t.flows()) {
            flow.grids(out);
        }
        if let Some(g) = &self.grid {
            out.push((self.id, g.items.iter().map(|i| i.id).collect()));
            for it in &g.items {
                if let Some(inner) = &it.inner {
                    inner.grids(out);
                }
            }
        }
        for k in self
            .kids
            .iter()
            .chain(self.flex.iter().flat_map(|f| &f.items))
        {
            if let Some(inner) = &k.inner {
                inner.grids(out);
            }
        }
    }

    /// Ключи таблиц стопки (`layout::table::grid_rows::table_key`): их
    /// раскладку рекордер записывает кадрами (`Laid::tables`).
    pub(crate) fn table_keys(&self) -> Vec<u64> {
        let mut out = Vec::new();
        self.push_keys(&mut out);
        out
    }

    /// Стопка блочного контейнера по определению — ячейки или подписи
    /// таблицы (CSS 2.1 §17.4, §17.5): дети кладутся блочной стопкой, какое
    /// бы значение `display` ни несла коробка.
    pub(crate) fn container(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> Self {
        let mut out = frame(e, cx, vcx);
        let mut adjoins = false;
        push_kids(&e.children, &mut out.kids, &mut adjoins, cx, vcx);
        out
    }
}
