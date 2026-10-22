//! Блочная стопка копии ребёнка фрагментации: какие коробки записывает рекордер раскладки
//! (`paginate::watch::Watched`) и какие свойства их границ читает фрагментация. Положения и
//! размеры берутся из РАЗЛОЖЕННЫХ border box и дорожек сетки таблиц (`laid`), а не из оценки
//! формы: из дерева — только то, что задаёт стиль (принудительные разрывы и запреты на
//! границах с переносом с крайних детей, монолитность, рамка и отбивка у краёв, корень
//! блочного контекста, устройство таблицы — `table`). Возможности разрыва класса A между
//! соседями-блоками (css-break-3 §4.3) и усечение полей на разрыве (§5.2) — в `geometry`.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flex_lines::class_a_box;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::page::names::page_names;
use crate::style::computed::{Display, LenCx};

mod finish;
mod geometry;
pub(crate) use geometry::{BlockBreaks, MeasuredBreaks};
mod laid;
pub(crate) use laid::{Laid, TableFrame};
mod trees;
use trees::{holds_blocks, stacks_blocks};
pub(crate) use trees::{stacked_tree, table_tree};
pub(crate) mod table;
use table::TableBox;
mod grid;
mod grid_breaks;
pub(crate) use grid::{GridBox, Pushed, RowPush, grid_container, grid_tree, item_rows};
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
    /// Устройство таблицы, если коробка — таблица (её обёртка): дети тогда
    /// пусты, границы дают подписи и ряды (`table`).
    pub(crate) table: Option<Box<TableBox>>,
    /// Элементы, если контейнер — сетка: детей стопкой она не кладёт.
    pub(crate) grid: Option<GridBox>,
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

    /// Нет ни блочных детей, ни таблицы, ни элементов сетки: рекордеру
    /// стопки нечего наблюдать.
    pub(crate) fn is_empty(&self) -> bool {
        self.kids.is_empty()
            && self.table.is_none()
            && self.grid.as_ref().is_none_or(|g| g.items.is_empty())
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
        if let Some(t) = &self.table {
            t.push_ids(out);
        }
    }

    /// Ключи таблиц стопки (`layout::table::grid_rows::table_key`): их
    /// раскладку рекордер записывает кадрами (`Laid::tables`).
    pub(crate) fn table_keys(&self) -> Vec<u64> {
        let mut out = Vec::new();
        self.push_keys(&mut out);
        out
    }

    fn push_keys(&self, out: &mut Vec<u64>) {
        if let Some(g) = &self.grid {
            for it in &g.items {
                if let Some(inner) = &it.inner {
                    inner.push_keys(out);
                }
            }
        }
        for k in &self.kids {
            if let Some(inner) = &k.inner {
                inner.push_keys(out);
            }
        }
        if let Some(t) = &self.table {
            t.push_keys(out);
        }
    }

    /// Стопка блочного контейнера по определению — ячейки или подписи
    /// таблицы (CSS 2.1 §17.4, §17.5): дети кладутся блочной стопкой, какое
    /// бы значение `display` ни несла коробка.
    pub(crate) fn container(e: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> Self {
        let mut out = frame(e, cx, vcx);
        if depth > 0 {
            let mut adjoins = false;
            push_kids(&e.children, &mut out.kids, &mut adjoins, depth, cx, vcx);
        }
        out
    }
}

/// `blockified` — элемент сетки: его `inline-block` — блок (css-display-3 §2.7).
pub(super) fn collect(
    e: &Element,
    depth: u8,
    blockified: bool,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) -> BlockBox {
    // Таблица горизонтального письма: обёртка без своих рамки и отбивки,
    // её границы — подписи и ряды (`table`). Вертикальную таблицу рекордер
    // не читает (блочные границы — только по горизонтальной оси).
    if table_box(e) && !e.style.wm().vertical() {
        return BlockBox {
            id: e.node_id,
            table: (depth > 0).then(|| Box::new(TableBox::collect(e, depth, cx, vcx))),
            ..Default::default()
        };
    }
    let mut out = frame(e, cx, vcx);
    if depth == 0 {
        return out;
    }
    if grid_container(e) {
        out.grid = Some(grid::collect_grid(e, depth - 1, cx, vcx));
        return out;
    }
    let stacks = stacks_blocks(e) || (blockified && e.style.display == Some(Display::InlineBlock));
    if !stacks {
        return out;
    }
    let mut adjoins = false;
    push_kids(&e.children, &mut out.kids, &mut adjoins, depth, cx, vcx);
    out
}

/// Коробка `e` без детей: рамка с отбивкой у краёв и корень контекста.
fn frame(e: &Element, cx: ShapeCx, vcx: ValueCx<'_>) -> BlockBox {
    let lens = LenCx::box_viewport(cx.viewport, &e.style);
    let px = |l| lens.px_lossy_zero(l, vcx);
    let b = e.style.borders();
    BlockBox {
        id: e.node_id,
        head: px(e.style.padding.top) + px(b.top),
        tail: px(e.style.padding.bottom) + px(b.bottom),
        end_margin: crate::layout::box_kind::bfc_root(e)
            && !grid_container(e)
            && matches!(
                e.style.height,
                None | Some(crate::style::values::value::Len::Auto)
            ),
        kids: Vec::new(),
        table: None,
        grid: None,
    }
}

/// Поточные блочные дети из узлов `nodes`. Блок внутри строчной коробки
/// разрывает её (CSS 2.1 §9.2.1.1): части строчной коробки уходят в
/// анонимные блоки, а сам блок — сосед блоков того же потока, поэтому
/// строчная коробка с блоками внутри раскрывается на месте.
fn push_kids(
    nodes: &[Node],
    out: &mut Vec<BlockKid>,
    adjoins: &mut bool,
    depth: u8,
    cx: ShapeCx,
    vcx: ValueCx<'_>,
) {
    for n in nodes.iter().filter(|n| !is_blank(n)) {
        let k = match n {
            // Внепоточные соседей не двигают (CSS 2.1 §9.3.1) и границу
            // соседей не прерывают.
            Node::Element(k) if out_of_flow(&k.style) => continue,
            Node::Element(k) if matches!(k.style.display, Some(Display::None)) => continue,
            Node::Element(k) if !k.inline && k.style.display != Some(Display::Contents) => k,
            Node::Element(k) if k.inline && k.style.display.is_none() && holds_blocks(k) => {
                push_kids(&k.children, out, adjoins, depth, cx, vcx);
                continue;
            }
            // Строчное содержимое между блоками — анонимный блок строк: его
            // границы дают измеренные строки, не эта стопка.
            _ => {
                *adjoins = false;
                continue;
            }
        };
        let monolith = monolith_box(k);
        out.push(BlockKid {
            id: k.node_id,
            adjoins: *adjoins,
            force_before: edge_break(k, false, vcx),
            force_after: edge_break(k, true, vcx),
            avoid_before: edge_avoid(k, false, vcx),
            avoid_after: edge_avoid(k, true, vcx),
            monolith,
            inner: (!monolith && (stacks_blocks(k) || table_box(k) || grid_container(k)))
                .then(|| collect(k, depth - 1, false, cx, vcx)),
            page: (cx.paged && class_a_box(k)).then(|| page_names(k, "")),
        });
        *adjoins = true;
    }
}
