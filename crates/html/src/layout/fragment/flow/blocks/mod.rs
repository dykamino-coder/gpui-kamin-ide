//! Блочная стопка копии ребёнка фрагментации: какие коробки записывает рекордер раскладки
//! (`paginate::watch::Watched`) и какие свойства их границ читает фрагментация. Положения и
//! размеры берутся из РАЗЛОЖЕННЫХ border box (`geometry`), а не из оценки формы: из
//! дерева — только то, что задаёт стиль (принудительные разрывы и запреты на границах с
//! переносом с крайних детей, монолитность, рамка и отбивка у краёв, корень блочного
//! контекста). Возможности разрыва класса A между соседями-блоками (css-break-3 §4.3) и
//! усечение полей на разрыве (§5.2) — в `geometry`.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::monolith_box;
use crate::layout::fragment::flex_lines::class_a_box;
use crate::layout::page::names::page_names;
use crate::style::computed::{Display, LenCx};
use crate::tolerance::COORD_EPS;

mod geometry;
pub(crate) use geometry::{BlockBreaks, MeasuredBreaks};
#[cfg(test)]
mod tests;

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
        collect(e, DEPTH, cx, vcx)
    }

    /// Номера узлов корня и всех коробок стопки (их раскладку пишет
    /// рекордер).
    pub(crate) fn ids(&self) -> Vec<u64> {
        let mut out = vec![self.id];
        self.push_ids(&mut out);
        out
    }

    fn push_ids(&self, out: &mut Vec<u64>) {
        for k in &self.kids {
            out.push(k.id);
            if let Some(inner) = &k.inner {
                inner.push_ids(out);
            }
        }
    }

    /// Самая внешняя коробка стопки, чей разложенный border box начинается
    /// ровно в `a` (`at(id)` — `(верх, низ)` в координатах потока): перед ней
    /// встаёт распорка роста. Коробка, накрывающая `a`, — спуск в её стопку;
    /// монолит и коробка без своей стопки спуска не дают.
    pub(crate) fn pushed_at(&self, a: f32, at: &dyn Fn(u64) -> Option<(f32, f32)>) -> Option<u64> {
        for k in &self.kids {
            let Some((top, bottom)) = at(k.id) else {
                continue;
            };
            if (top - a).abs() < COORD_EPS {
                return Some(k.id);
            }
            if top < a && a < bottom - COORD_EPS {
                return k.inner.as_ref()?.pushed_at(a, at);
            }
        }
        None
    }
}

/// Блочный контейнер, чьи дети идут стопкой по блочной оси: блок и элемент
/// списка. Flex, сетка, таблица, многоколоночник и строчный набор кладут
/// детей иначе — их границы дают свои контексты (строки — `Paragraph`).
fn stacks_blocks(e: &Element) -> bool {
    matches!(
        e.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) && e.style.webkit_box != Some(true)
        && !crate::layout::multicol::spanner::multicol_container(&e.style)
        && !crate::layout::fragment::table_bands::table_box(e)
        && !matches!(e.tag.as_str(), "tr" | "thead" | "tbody" | "tfoot")
}

/// Всё поточное блочное поддерево `e` до глубины `depth` — блочные стопки и
/// монолиты: каждую коробку, перед которой может встать распорка, рекордер
/// стопки видит (таблица, flex и сетка кладут детей иначе).
pub(crate) fn stacked_tree(e: &Element, depth: u8) -> bool {
    stacks_blocks(e)
        && (depth == 0
            || e.children.iter().all(|n| match n {
                Node::Element(k) if out_of_flow(&k.style) || k.inline => {
                    !holds_blocks(k) || stacked_tree(k, depth - 1)
                }
                Node::Element(k) => monolith_box(k) || stacked_tree(k, depth - 1),
                _ => true,
            }))
}

fn collect(e: &Element, depth: u8, cx: ShapeCx, vcx: ValueCx<'_>) -> BlockBox {
    let lens = LenCx::box_viewport(cx.viewport, &e.style);
    let px = |l| lens.px_lossy_zero(l, vcx);
    let b = e.style.borders();
    let mut out = BlockBox {
        id: e.node_id,
        head: px(e.style.padding.top) + px(b.top),
        tail: px(e.style.padding.bottom) + px(b.bottom),
        end_margin: crate::layout::box_kind::bfc_root(e)
            && matches!(
                e.style.height,
                None | Some(crate::style::values::value::Len::Auto)
            ),
        kids: Vec::new(),
    };
    if depth == 0 || !stacks_blocks(e) {
        return out;
    }
    let mut adjoins = false;
    push_kids(&e.children, &mut out.kids, &mut adjoins, depth, cx, vcx);
    out
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
            inner: (!monolith && stacks_blocks(k)).then(|| collect(k, depth - 1, cx, vcx)),
            page: (cx.paged && class_a_box(k)).then(|| page_names(k, "")),
        });
        *adjoins = true;
    }
}

/// Строчная коробка с поточным блоком внутри (на любой глубине строчных).
fn holds_blocks(e: &Element) -> bool {
    e.children.iter().any(|n| match n {
        Node::Element(k) if out_of_flow(&k.style) => false,
        Node::Element(k) if !k.inline => true,
        Node::Element(k) => k.style.display.is_none() && holds_blocks(k),
        _ => false,
    })
}
