//! Разложенные секции повтора таблицы (css-tables-3 §repeated-headers): полосы первой
//! шапки и первого подвала по дорожкам рядов разложенной сетки, зазор `border-spacing` и
//! коробка таблицы — вход общего правила повтора (`table_bands::repeat_spec`).

use super::TableBox;
use crate::layout::fragment::flow::blocks::Laid;

/// Разложенные секции повтора таблицы (css-tables-3 §repeated-headers): полосы
/// первой шапки и первого подвала `(верх, высота, break-inside: avoid)`, зазор
/// `border-spacing` между рядами и коробка таблицы `[box_top, box_end)` — всё
/// в координатах копии.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TableSections {
    pub(crate) head: Option<(f32, f32, bool)>,
    pub(crate) foot: Option<(f32, f32, bool)>,
    pub(crate) spacing: f32,
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
        Some(TableSections {
            head: band(self.head),
            foot: band(self.foot),
            // Зазор между рядами — он же отбивка сетки над первым рядом.
            spacing: f.rows.first().map_or(0.0, |r| r.0 - f.grid.0),
            box_top: f.table.0,
            box_end: f.table.1,
        })
    }
}
