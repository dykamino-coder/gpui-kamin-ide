//! Логические ряды таблицы: обход групп и рядов по тегу или `display` с переносом
//! относительного сдвига, фона группы и самой группы (`RowCarry`) в каждый ряд; ряды,
//! схлопнутые `visibility: collapse`, сохраняются для внутренних размеров и `rowspan`.
use crate::dom::{Element, Node};
use crate::layout::positioned::relative::relative_shift;
use crate::style::computed::Display;
/// Сдвиг, фон и САМА ГРУППА рядов: от неё нужны и наследуемый стиль, и
/// `node_id` с рамками — кромки группы строит ряд.
#[derive(Clone, Copy, Default)]
pub(in crate::layout::table) struct RowCarry<'a> {
    pub dx: f32,
    pub dy: f32,
    pub background: Option<crate::style::values::value::Color>,
    pub group: Option<&'a Element>,
}

pub(in crate::layout::table) struct TableRow<'a> {
    pub element: &'a Element,
    pub carry: RowCarry<'a>,
}

pub(in crate::layout::table) fn collect_rows<'a>(
    nodes: &'a [Node],
    parent: Option<&'a Element>,
    carry: RowCarry<'a>,
    out: &mut Vec<TableRow<'a>>,
) {
    for n in nodes {
        if let Node::Element(e) = n {
            let (dx, dy) = relative_shift(e, parent);
            // Фон группы строк рисуют ЯЧЕЙКИ: своей коробки у группы в общей
            // сетке не остаётся, и заливка пропадала молча
            // (`position-relative-table-tbody-left`: зелёная коробка не
            // рисовалась вовсе, из-под неё светило красное).
            let shift = RowCarry {
                dx: carry.dx + dx,
                dy: carry.dy + dy,
                background: e.style.background.or(carry.background),
                group: carry.group,
            };
            // Retain collapsed rows through intrinsic sizing and rowspan placement.
            // Роль задаётся тегом ИЛИ стилем: разметка на `div` с
            // `display: table-row` встречается не реже настоящих таблиц.
            if e.tag == "tr" || e.style.display == Some(Display::TableRow) {
                out.push(TableRow {
                    element: e,
                    carry: shift,
                });
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
                let deeper = RowCarry {
                    background: if picture { None } else { shift.background },
                    group: Some(e),
                    ..shift
                };
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
        collect_rows(&nodes, None, RowCarry::default(), &mut rows);
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|row| row_collapsed(row.element, row.carry.group))
        );
    }

    #[test]
    fn row_records_keep_both_shift_axes_background_and_group() {
        use crate::style::computed::Position;
        use crate::style::values::value::{Color, Len};
        let mut row = anon_element("tr", vec![]);
        row.style.position = Some(Position::Relative);
        row.style.inset.left = Some(Len::Px(3.0));
        row.style.inset.top = Some(Len::Px(7.0));
        let mut group = anon_element("tbody", vec![Node::Element(row)]);
        group.style.position = Some(Position::Relative);
        group.style.inset.left = Some(Len::Px(11.0));
        group.style.inset.top = Some(Len::Px(17.0));
        let background = Color {
            r: 0.0,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        };
        group.style.background = Some(background);
        let nodes = vec![Node::Element(group)];
        let mut rows = Vec::new();
        collect_rows(
            &nodes,
            None,
            RowCarry {
                dx: 19.0,
                dy: 23.0,
                ..Default::default()
            },
            &mut rows,
        );
        assert_eq!(rows.len(), 1);
        let record = &rows[0];
        assert_eq!(record.element.tag, "tr");
        assert_eq!(record.carry.dx, 33.0);
        assert_eq!(record.carry.dy, 47.0);
        assert_eq!(record.carry.background, Some(background));
        let Node::Element(group) = &nodes[0] else {
            unreachable!()
        };
        assert!(std::ptr::eq(record.carry.group.unwrap(), group));
    }
}
