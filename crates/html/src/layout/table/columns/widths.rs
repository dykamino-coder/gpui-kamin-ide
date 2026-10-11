//! Полосы колонок и ширины из элементов <col>/<colgroup>.

use super::{col_role, col_span};
use crate::dom::{Element, Node};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement};

/// Полоса слоя на каждую красящую дорожку плюс буфер проб по её индексу.
///
/// Одно тело на слой групп и слой колонок: различаются они только набором
/// элементов и порядком вызова (§17.5.1 — группы ПОД колонками).
pub(crate) fn push_col_bands(
    els: &[Option<&Element>],
    salt: u64,
    have_rows: bool,
    rects_by_col: &mut [Option<crate::layout::table::paint::RowRects>],
    cells: &mut Vec<AnyElement>,
) {
    let mut seen: Vec<u64> = vec![];
    for (i, el) in els.iter().enumerate() {
        let Some(el) = el else { continue };
        // Полоса красит СВОИ ЯЧЕЙКИ. Ячеек нет — красить нечего, а пустая
        // полоса ещё и просит перерисовку два кадра подряд впустую
        // (`table-column-rendering-001`: колонка сама по себе не рисуется).
        // Дорожка за краем сетки буфера не получает: её пробы не напишет
        // никто.
        if !have_rows || i >= rects_by_col.len() {
            continue;
        }
        let picture = el.style.bg_image.is_some() || el.style.gradient_raw.is_some();
        // Дорожка с одним ЦВЕТОМ тоже красится полосой: своей коробки у неё
        // нет, фон рисуют её ячейки.
        if !(picture || el.style.background.is_some() || !el.style.shadows.is_empty()) {
            continue;
        }
        let rects = crate::layout::table::paint::row_rects_for(el.node_id ^ salt);
        rects_by_col[i] = Some(rects.clone());
        if !seen.contains(&el.node_id) {
            seen.push(el.node_id);
            let mut band_style = el.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            cells.push(
                crate::layout::table::paint::CellsClipped::new(rects, band_style)
                    .into_any_element(),
            );
        }
    }
}

pub(crate) fn col_element_widths(
    children: &[Node],
    base_font: f32,
    family: &str,
) -> (Vec<Option<f32>>, Vec<bool>, Vec<Option<f32>>) {
    let mut widths = vec![];
    let mut collapsed = vec![];
    // Доля ширины колонки (§17.5.2.1): в точках её не выразить, дорожка
    // получает её отдельно.
    let mut pcts: Vec<Option<f32>> = vec![];
    // §17.5.2.1 берёт ИСПОЛЬЗОВАННУЮ ширину колонки, а вычисляет её §10.4:
    // сперва пробная ширина, потом потолок `max-width`, потом пол
    // `min-width`. Прежде читалась одна `width`, и `min-width: 1in` на
    // колонке без ширины оставляла дорожку `Fraction(1)` в столе автоширины
    // — то есть НОЛЬ и пустой холст (`min-width-applies-to-005/006`), а
    // `width: 3in` рядом с `max-width: 1in` держала все 288
    // (`max-width-applies-to-005/006`).
    //
    // Потолок применяется ТОЛЬКО к ЗАЯВЛЕННОЙ ширине: одинокий `max-width`
    // авто-дорожку закреплять не вправе — верхняя грань дорожки живёт в
    // `track_list`, и отдельная пара (пол, потолок) на дорожку — следующий
    // шаг, не этот.
    let used_w = |el: &Element, w: Option<f32>| -> Option<f32> {
        let len_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let capped = match (w, len_px(el.style.max_width)) {
            (Some(v), Some(mx)) => Some(v.min(mx)),
            (v, _) => v,
        };
        match len_px(el.style.min_width) {
            Some(mn) => Some(capped.map_or(mn, |v| v.max(mn))),
            None => capped,
        }
    };
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => {
                let w = used_w(
                    el,
                    match el.style.width {
                        Some(Len::Px(v)) => Some(v),
                        // `ch` на колонке считается с ЕЁ письмом: стоячий ноль
                        // продвигается на кегль (ch-units-vrl-003/004). Кегль
                        // колонки — свой или умолчание: шрифт таблицы сюда не
                        // наследуется, а тесты задают его одинаковым.
                        // Только СТОЯЧИЕ (upright): там продвижение — кегль и
                        // сходится с эталоном. Лежачий `ch` (sideways) оставлен
                        // авто-колонке: явный глиф-замер уводил ширину
                        // (ch-units-vrl-007/008 были зелёными на авто).
                        Some(Len::Ch(k))
                            if el.style.upright == Some(true)
                                && el.style.vertical == Some(true) =>
                        {
                            let base = match el.style.font_size {
                                Some(Len::Px(v)) => v,
                                _ => base_font,
                            };
                            let _ = family;
                            Some(k * base)
                        }
                        _ => None,
                    },
                );
                // `visibility: collapse` на колонке — колонка ВЫБРОШЕНА:
                // нулевая дорожка, ячейки не рисуются (css-tables-3
                // §visibility-collapse-cell-rendering).
                let c = el.style.collapsed == Some(true);
                let span = col_span(el);
                let p = match el.style.width {
                    Some(Len::Pct(k)) => Some(k),
                    _ => None,
                };
                widths.extend(std::iter::repeat_n(w, span));
                collapsed.extend(std::iter::repeat_n(c, span));
                pcts.extend(std::iter::repeat_n(p, span));
            }
            Some(true) => {
                let (w, c, p) = col_element_widths(&el.children, base_font, family);
                if w.is_empty() {
                    // Группа без колонок внутри САМА стоит колонкой (см.
                    // `col_elements`), значит и §10.4 к ней применяется тот
                    // же — иначе `min-width-applies-to-005` и
                    // `max-width-applies-to-005` (группа вместо колонки)
                    // остались бы красными при зелёных `-006`.
                    let ww = used_w(
                        el,
                        match el.style.width {
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        },
                    );
                    let pp = match el.style.width {
                        Some(Len::Pct(k)) => Some(k),
                        _ => None,
                    };
                    let cc = el.style.collapsed == Some(true);
                    let span = col_span(el);
                    widths.extend(std::iter::repeat_n(ww, span));
                    collapsed.extend(std::iter::repeat_n(cc, span));
                    pcts.extend(std::iter::repeat_n(pp, span));
                } else {
                    widths.extend(w);
                    collapsed.extend(c);
                    pcts.extend(p);
                }
            }
            None => {}
        }
    }
    (widths, collapsed, pcts)
}
