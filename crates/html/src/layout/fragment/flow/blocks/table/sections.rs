//! Разложенные секции повтора таблицы (css-tables-3 §repeated-headers): полосы первой
//! шапки и первого подвала по дорожкам рядов разложенной сетки, зазор `border-spacing`,
//! края коробки при сросшихся рамках, концы рядов и коробка таблицы — вход общего правила
//! повтора (`table_bands::repeat_spec`).

use super::TableBox;
use crate::layout::fragment::flow::blocks::Laid;

/// Разложенные секции повтора таблицы (css-tables-3 §repeated-headers): полосы
/// первой шапки и первого подвала `(верх, высота, break-inside: avoid)`, зазор
/// `border-spacing` между рядами, концы рядов и коробка таблицы
/// `[box_top, box_end)` — всё в координатах копии.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TableSections {
    pub(crate) head: Option<(f32, f32, bool)>,
    pub(crate) foot: Option<(f32, f32, bool)>,
    pub(crate) spacing: f32,
    /// Рамка и отбивка коробки над сеткой и под ней, которые несёт полоса
    /// повтора при сросшихся рамках: повторённая шапка рисует верхнюю рамку
    /// таблицы целиком, повторённый подвал — нижнюю (css-tables-3
    /// §repeated-headers: «leave room and if needed render the table top
    /// border. The same applies for footer rows and the table bottom border»;
    /// Blink `table_layout_algorithm.cc` резервирует `border_padding` только при
    /// `has_collapsed_borders`). При раздельных рамках — нули: край коробки
    /// повторять нечем (`box-decoration-break: slice`).
    pub(crate) edge_top: f32,
    pub(crate) edge_bottom: f32,
    /// Концы рядов сетки по порядку дорожек.
    pub(crate) row_ends: Vec<f32>,
    pub(crate) box_top: f32,
    pub(crate) box_end: f32,
}

impl TableBox {
    /// Разложенные секции повтора таблицы.
    pub(in crate::layout::fragment::flow::blocks) fn sections(
        &self,
        laid: &Laid,
    ) -> Option<TableSections> {
        let f = laid.table(self.key)?;
        let band = |s: Option<(usize, usize, bool)>| {
            let (first, last, avoid) = s?;
            let top = f.rows.get(first)?.0;
            Some((top, f.rows.get(last)?.1 - top, avoid))
        };
        let (edge_top, edge_bottom) = if self.collapsed {
            (
                (f.grid.0 - f.table.0).max(0.0),
                (f.table.1 - f.grid.1).max(0.0),
            )
        } else {
            (0.0, 0.0)
        };
        Some(TableSections {
            head: band(self.head),
            foot: band(self.foot),
            // Зазор между рядами — он же отбивка сетки над первым рядом.
            spacing: f.rows.first().map_or(0.0, |r| r.0 - f.grid.0),
            edge_top,
            edge_bottom,
            row_ends: f.rows.iter().map(|r| r.1).collect(),
            box_top: f.table.0,
            box_end: f.table.1,
        })
    }
}
