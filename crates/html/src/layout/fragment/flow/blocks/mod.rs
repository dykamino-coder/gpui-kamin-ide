//! Блочная стопка копии ребёнка фрагментации: какие коробки записывает рекордер раскладки
//! (`paginate::watch::Watched`) и какие свойства их границ читает фрагментация. Положения и
//! размеры берутся из РАЗЛОЖЕННЫХ border box (`geometry`), а не из оценки формы: из
//! дерева — только то, что задаёт стиль (принудительные разрывы и запреты на границах с
//! переносом с крайних детей, монолитность, рамка и отбивка у краёв, корень блочного
//! контекста). Возможности разрыва класса A между соседями-блоками (css-break-3 §4.3) и
//! усечение полей на разрыве (§5.2) — в `geometry`.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::tolerance::COORD_EPS;

mod collect;
use collect::collect;
pub(crate) use collect::stacked_tree;
mod geometry;
pub(crate) use geometry::{BlockBreaks, MeasuredBreaks};
mod grid;
mod grid_breaks;
pub(crate) use grid::{GridBox, Pushed, RowPush, grid_container, item_rows};
mod grid_tree;
pub(crate) use grid_tree::grid_tree;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod tests;

/// Номер распорки роста перед коробкой `id` — `id ^ STRUT_ID`
/// (`push::spacer::spacer_before`): рекордер стопки пишет и её, и её низ —
/// граница, на которой встаёт разрыв перед коробкой.
pub(crate) const STRUT_ID: u64 = 0x5350_4143_4552_0001;

/// Глубина спуска — та же, что у меры стопки (`shape_full(c, 4, ..)`) плюс
/// корень копии.
const DEPTH: u8 = 5;

/// Блочный контейнер стопки: его рамка с отбивкой у краёв и поточные
/// блочные дети в порядке потока.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BlockBox {
    pub(crate) id: u64,
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
    /// Элементы, если контейнер — сетка: детей стопкой она не кладёт.
    pub(crate) grid: Option<GridBox>,
}

/// Разложенная геометрия копии, прочитанная до освобождения движка раскладки:
/// border box `(верх, низ)` наблюдаемых коробок и дорожки рядов сеток
/// `(начало, конец)` — в координатах потока.
pub(crate) struct Laid<'a> {
    pub(crate) at: &'a dyn Fn(u64) -> Option<(f32, f32)>,
    pub(crate) rows: &'a dyn Fn(u64) -> Option<Vec<(f32, f32)>>,
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
        collect(e, DEPTH, false, cx, vcx)
    }

    /// Номера узлов корня и всех коробок стопки (их раскладку пишет
    /// рекордер).
    pub(crate) fn ids(&self) -> Vec<u64> {
        let mut out = vec![self.id];
        self.push_ids(&mut out);
        out
    }

    /// Нет ни блочных детей, ни элементов сетки: рекордеру стопки нечего
    /// наблюдать.
    pub(crate) fn is_empty(&self) -> bool {
        self.kids.is_empty() && self.grid.as_ref().is_none_or(|g| g.items.is_empty())
    }

    /// Сетки стопки: номер контейнера и номера его элементов — дорожки рядов
    /// читаются у разложенного элемента (`parent_grid_tracks`).
    pub(crate) fn grids(&self, out: &mut Vec<(u64, Vec<u64>)>) {
        if let Some(g) = &self.grid {
            out.push((self.id, g.items.iter().map(|i| i.id).collect()));
            for it in &g.items {
                if let Some(inner) = &it.inner {
                    inner.grids(out);
                }
            }
        }
        for k in &self.kids {
            if let Some(inner) = &k.inner {
                inner.grids(out);
            }
        }
    }

    fn push_ids(&self, out: &mut Vec<u64>) {
        if let Some(g) = &self.grid {
            g.push_ids(out);
        }
        for k in &self.kids {
            out.push(k.id);
            if let Some(inner) = &k.inner {
                inner.push_ids(out);
            }
        }
    }

    /// Самая внешняя коробка стопки, чей разложенный border box начинается
    /// ровно в `a` (`laid.at(id)` — `(верх, низ)` в координатах потока): перед
    /// ней встаёт распорка роста. Коробка, накрывающая `a`, — спуск в её
    /// стопку; монолит и коробка без своей стопки спуска не дают. В сетке —
    /// ряд, начинающийся в `a` (`GridBox::pushed_at`).
    pub(crate) fn pushed_at(&self, a: f32, laid: &Laid<'_>) -> Option<Pushed> {
        if let Some(g) = &self.grid {
            return g.pushed_at(self.id, a, laid);
        }
        for k in &self.kids {
            let Some((top, bottom)) = (laid.at)(k.id) else {
                continue;
            };
            if (top - a).abs() < COORD_EPS {
                return Some(Pushed::Box(k.id));
            }
            if top < a && a < bottom - COORD_EPS {
                return k.inner.as_ref()?.pushed_at(a, laid);
            }
        }
        None
    }
}
