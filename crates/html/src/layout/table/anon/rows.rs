//! Логические ряды таблицы: обход групп и рядов по тегу или `display` с переносом
//! относительного сдвига, фона группы и самой группы (`RowCarry`) в каждый ряд; ряды,
//! схлопнутые `visibility: collapse`, сохраняются для внутренних размеров и `rowspan`.
use crate::dom::{Element, Node};
use crate::layout::positioned::relative::relative_shift;
use crate::style::computed::Display;
/// Сдвиг, фон и СТИЛЬ ГРУППЫ строк: письмо/шрифт с `<tbody>` наследуются в
/// ряды и ячейки, хотя своей коробки у группы нет (ch-units-vrl-006).
/// Сдвиг, фон и САМА ГРУППА рядов: от неё нужны и наследуемый стиль, и
/// `node_id` с рамками — кромки группы строит ряд.
pub(in crate::layout::table) type RowCarry<'a> = (
    f32,
    f32,
    Option<crate::style::values::value::Color>,
    Option<&'a Element>,
);

pub(in crate::layout::table) fn collect_rows<'a>(
    nodes: &'a [Node],
    parent: Option<&'a Element>,
    carry: RowCarry<'a>,
    out: &mut Vec<(&'a Element, RowCarry<'a>)>,
) {
    for n in nodes {
        if let Node::Element(e) = n {
            let (dx, dy) = relative_shift(e, parent);
            // Фон группы строк рисуют ЯЧЕЙКИ: своей коробки у группы в общей
            // сетке не остаётся, и заливка пропадала молча
            // (`position-relative-table-tbody-left`: зелёная коробка не
            // рисовалась вовсе, из-под неё светило красное).
            let shift = (
                carry.0 + dx,
                carry.1 + dy,
                e.style.background.or(carry.2),
                carry.3,
            );
            // Retain collapsed rows through intrinsic sizing and rowspan placement.
            // Роль задаётся тегом ИЛИ стилем: разметка на `div` с
            // `display: table-row` встречается не реже настоящих таблиц.
            if e.tag == "tr" || e.style.display == Some(Display::TableRow) {
                out.push((e, shift));
            } else if e.tag == "thead"
                || e.tag == "tbody"
                || e.tag == "tfoot"
                || e.style.display == Some(Display::TableRowGroup)
            {
                // Группа с КАРТИНКОЙ красит и цвет САМА — полосой `grp_band`
                // (§14.2: цвет лежит ПОД картинкой). Ячейка его не дублирует:
                // она рисуется после полосы, и цвет прятал бы картинку
                // (`background-repeat-applies-to-001/002/003`). У ряда та же
                // отсечка стоит давно.
                //
                // Отсечка держится на том, что полоса знает
                // `attachment: fixed`: без этого ломается тройка
                // `background-attachment-applies-to-*`.
                let picture = e.style.bg_image.is_some() || e.style.gradient_raw.is_some();
                let deeper = (
                    shift.0,
                    shift.1,
                    if picture { None } else { shift.2 },
                    Some(e),
                );
                collect_rows(&e.children, Some(e), deeper, out);
            }
        }
    }
}

/// Row-group collapse applies even when a row specifies visible.
pub(in crate::layout::table) fn row_collapsed(row: &Element, group: Option<&Element>) -> bool {
    row.style.collapsed == Some(true) || group.is_some_and(|g| g.style.collapsed == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::anon_element;

    #[test]
    fn logical_rows_retain_collapsed_row_and_group() {
        let mut collapsed = anon_element("tr", vec![]);
        collapsed.style.collapsed = Some(true);
        let visible = anon_element("tr", vec![]);
        let mut group = anon_element("tbody", vec![Node::Element(visible)]);
        group.style.collapsed = Some(true);
        let nodes = vec![Node::Element(collapsed), Node::Element(group)];
        let mut rows = Vec::new();
        collect_rows(&nodes, None, (0.0, 0.0, None, None), &mut rows);
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(row, carry)| row_collapsed(row, carry.3)));
    }
}
